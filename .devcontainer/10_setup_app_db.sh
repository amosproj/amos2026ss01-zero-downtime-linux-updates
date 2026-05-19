#!/usr/bin/env bash
set -e

db_name="amos"
db_user="app"
db_password="4M0S"

psql --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<EOF
    CREATE USER $db_user;
    ALTER USER $db_user PASSWORD '$db_password';
    CREATE DATABASE $db_name;

    \connect $db_name;
    GRANT USAGE, CREATE ON SCHEMA public TO $db_user;
    GRANT ALL PRIVILEGES ON DATABASE $db_name TO $db_user;
EOF
