# statussentinel
An open-source web status monitoring tool written in Rust.

## Features
- Monitor the status of your web services and minecraft servers
- Start incidents when a service is down with its error code
- Store all data in a postgres database for easy access and reporting
- Get detailed reports of your services and minecraft servers

## Installation

### Prerequisites
- Rust and Cargo (latest stable version)
- PostgreSQL 15 or higher

### Step-by-step Installation

1. **Install Rust and Cargo**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```

2. **Install PostgreSQL**
   ```bash
   # For Ubuntu/Debian
   sudo apt update
   sudo apt install postgresql postgresql-contrib

   # For Fedora
   sudo dnf install postgresql postgresql-server
   sudo postgresql-setup --initdb
   sudo systemctl start postgresql
   sudo systemctl enable postgresql
   ```

3. **Setup PostgreSQL Database**
   ```bash
   # Login to PostgreSQL as postgres user
   sudo -u postgres psql

   # Create database and user
   CREATE DATABASE status_data;
   CREATE USER status_data_user WITH ENCRYPTED PASSWORD 'your_password';
   GRANT ALL PRIVILEGES ON DATABASE status_data TO status_data_user;
   \q
   ```

4. **Clone and Build the Repository**
   ```bash
   git clone https://github.com/tn3w/statussentinel.git
   cd statussentinel
   ```

5. **Configure Environment**
   ```bash
   cp .env.example .env
   # Edit .env with your database credentials
   ```

6. **Build and Run**
   ```bash
   cargo build --release
   cargo run --release
   ```

### .env file
Create a `.env` file in the root directory with the following variables:

Example `.env`:
```env
DATABASE_HOST=127.0.0.1
DATABASE_PORT=5432
DATABASE_NAME=status_data
DATABASE_USER=status_data_user
DATABASE_PASSWORD=your_password
```

### services.json file
Create a `services.json` file in the root directory to configure the services you want to monitor. The file should be a JSON object where keys are service names and values are URLs or connection strings.

Supported protocols:
- HTTP/HTTPS endpoints (use `/ping` endpoint for health checks)
- Minecraft servers (use `mc://` prefix port)

Example `services.json`:
```json
{
    "Main Website": "https://www.example.com/ping",
    "Secondary Website": "https://secondary.example.com/ping",
    "Minecraft Server": "mc://minecraft.example.com:25565"
}
```