use std::{
    env, fs,
    time::Duration,
    collections::HashMap,
    sync::Arc,
};

use tokio;
use tokio::time::sleep;

use dotenv::dotenv;
use serde_json::from_str;

mod database;
use database::{DbPool, Services, MonitoringError, init_database, format_service_id};

mod request;
use request::{ResponseResult, get_minecraft_response_time, get_request_response_time};


static LOGO: &str = r#"
       __       __                        __  _          __
  ___ / /____ _/ /___ _____ ___ ___ ___  / /_(_)__  ___ / /
 (_-</ __/ _ `/ __/ // (_-<(_-</ -_) _ \/ __/ / _ \/ -_) / 
/___/\__/\_,_/\__/\_,_/___/___/\__/_//_/\__/_/_//_/\__/_/  

An open-source web status monitoring tool written in Rust.

Author: TN3W
GitHub: https://github.com/tn3w/statussentinel
"#;


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    println!("{}", LOGO);

    let host = env::var("DATABASE_HOST").expect("DATABASE_HOST must be set");
    let port = env::var("DATABASE_PORT")
        .expect("DATABASE_PORT must be set")
        .parse::<u16>()
        .expect("DATABASE_PORT must be a valid port number");
    let dbname = env::var("DATABASE_NAME").expect("DATABASE_NAME must be set");
    let user = env::var("DATABASE_USER").expect("DATABASE_USER must be set");
    let password = env::var("DATABASE_PASSWORD").expect("DATABASE_PASSWORD must be set");

    let db_pool = DbPool::new(host, port, dbname, user, password).await?;
    init_database(&db_pool).await?;

    println!("*  Database connection established successfully!");

    let services_json = fs::read_to_string("services.json").expect("Failed to read services.json file");
    let services: Services = from_str(&services_json).expect("Failed to parse services.json");
    
    let mut added_services_count = 0;

    for (name, url) in &services.services {
        if let Err(e) = db_pool.add_service(name, url).await {
            eprintln!("Error adding service {}: {}", name, e);
        } else {
            added_services_count += 1;
        }
    }

    if added_services_count > 0 {
        println!("*  Services added successfully!");
    }

    println!("*  Starting status monitoring...");
    println!("*  Press Ctrl+C to stop.");

    run_monitoring_loop(&db_pool).await?;

    Ok(())
}

async fn run_monitoring_loop(db_pool: &DbPool) -> Result<(), MonitoringError> {
    #[derive(Clone)]
    struct ServiceState {
        has_open_incident: bool,
    }

    let service_states = HashMap::new();
    let service_states = Arc::new(tokio::sync::Mutex::new(service_states));

    loop {
        let services = db_pool.list_services().await?;
        
        {
            let mut states = service_states.lock().await;
            for service in &services {
                if !states.contains_key(&service.name) {
                    states.insert(service.name.clone(), ServiceState {
                        has_open_incident: false,
                    });
                }
            }
        }

        let mut monitoring_tasks = Vec::new();

        for service in &services {
            let url = service.server_url.clone();
            let name = service.name.clone();
            let db_pool = db_pool.clone();
            let service_states = service_states.clone();

            let monitoring_task = tokio::spawn(async move {
                let response_time = if url.starts_with("mc://") {
                    let server_addr = url.trim_start_matches("mc://");
                    let (host, port) = match server_addr.split_once(':') {
                        Some((h, p)) => (h, p.parse::<u16>().unwrap_or(25565)),
                        None => (server_addr, 25565)
                    };
                    get_minecraft_response_time(host, port)
                        .map_err(|e| MonitoringError(e.to_string()))? as i32
                } else {
                    match get_request_response_time(&url)
                        .await
                        .map_err(|e| MonitoringError(e.to_string()))? {
                        ResponseResult::Success(time) => time,
                        ResponseResult::StatusError(_) => 0
                    }
                };

                match format_service_id(&name) {
                    Ok(service_id) => {
                        if let Err(e) = db_pool.add_response_time(&service_id, response_time).await {
                            eprintln!("Error adding response time for {}: {}", name, e);
                            return Ok::<_, MonitoringError>(());
                        }

                        let recent_failures = db_pool.count_recent_failures(&service_id, 5).await?;

                        let mut states = service_states.lock().await;
                        let state = states.get_mut(&name).unwrap();

                        if response_time == 0 {
                            if recent_failures >= 5 && !state.has_open_incident {
                                if let Ok(incidents) = db_pool.list_incidents(false).await {
                                    let has_open_incident = incidents.iter().any(|i| i.service_id == service_id);
                                    if !has_open_incident {
                                        let incident_msg = match get_request_response_time(&url).await {
                                            Ok(ResponseResult::StatusError(status)) => {
                                                format!("Service {} is down: HTTP {} error", name, status)
                                            }
                                            _ => format!("Service {} is down after 5 consecutive failures", name)
                                        };

                                        if db_pool.add_incident(&service_id, &incident_msg).await.is_ok() {
                                            state.has_open_incident = true;
                                        }
                                    } else {
                                        state.has_open_incident = true;
                                    }
                                }
                            }
                        } else {
                            if state.has_open_incident {
                                if let Ok(incidents) = db_pool.list_incidents(false).await {
                                    for incident in incidents {
                                        if incident.service_id == service_id {
                                            db_pool.end_incident(incident.id).await.ok();
                                        }
                                    }
                                }
                                state.has_open_incident = false;
                            }
                        }
                    }
                    Err(e) => eprintln!("Error formatting service ID for {}: {}", name, e),
                }

                Ok::<_, MonitoringError>(())
            });

            monitoring_tasks.push(monitoring_task);
        }

        for task in monitoring_tasks {
            if let Err(e) = task.await {
                eprintln!("Error in monitoring task: {}", e);
            }
        }

        sleep(Duration::from_secs(60)).await;
    }
}