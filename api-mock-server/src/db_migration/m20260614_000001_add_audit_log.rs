use sea_orm_migration::prelude::*;

use crate::dtos;

use super::create_table;

const ID_PK_TABLES: &[&str] = &[
    "tenants",
    "groups",
    "devices",
    "applications",
    "application_configs",
    "application_assignments",
    "os_versions",
    "os_assignments",
    "reported_application_assignments",
    "reported_os_assignments",
];

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = sea_orm::Schema::new(manager.get_database_backend());

        create_table(manager, &schema, dtos::AuditLog::Entity).await?;

        if manager.get_database_backend() != sea_orm::DbBackend::Postgres {
            return Ok(());
        }

        let db = manager.get_connection();

        db.execute_unprepared(
            "INSERT INTO users (subject, name) VALUES ('system', 'System') ON CONFLICT (subject) DO NOTHING",
        )
        .await?;

        db.execute_unprepared(
            "ALTER TABLE audit_log ADD CONSTRAINT fk_audit_log_changed_by FOREIGN KEY (changed_by) REFERENCES users(id)",
        )
        .await?;

        db.execute_unprepared("ALTER TABLE audit_log ALTER COLUMN changed_at SET DEFAULT now()")
            .await?;

        db.execute_unprepared(
            "CREATE INDEX idx_audit_log_table_record ON audit_log (table_name, record_id)",
        )
        .await?;

        db.execute_unprepared("CREATE INDEX idx_audit_log_changed_at ON audit_log (changed_at)")
            .await?;

        db.execute_unprepared("CREATE INDEX idx_audit_log_changed_by ON audit_log (changed_by)")
            .await?;

        db.execute_unprepared(
            r#"
            CREATE OR REPLACE FUNCTION audit_log_trigger_fn()
            RETURNS TRIGGER AS $$
            DECLARE
                _user_id INTEGER;
            BEGIN
                BEGIN
                    _user_id := current_setting('app.audit_user', true)::integer;
                EXCEPTION WHEN OTHERS THEN
                    _user_id := NULL;
                END;

                IF _user_id IS NULL THEN
                    SELECT id INTO _user_id FROM users WHERE subject = 'system';
                END IF;

                IF (TG_OP = 'INSERT') THEN
                    INSERT INTO audit_log (table_name, record_id, operation, old_data, new_data, changed_by)
                    VALUES (TG_TABLE_NAME, NEW.id::TEXT, 'INSERT', NULL, to_jsonb(NEW), _user_id);
                ELSIF (TG_OP = 'UPDATE') THEN
                    INSERT INTO audit_log (table_name, record_id, operation, old_data, new_data, changed_by)
                    VALUES (TG_TABLE_NAME, NEW.id::TEXT, 'UPDATE', to_jsonb(OLD), to_jsonb(NEW), _user_id);
                ELSIF (TG_OP = 'DELETE') THEN
                    INSERT INTO audit_log (table_name, record_id, operation, old_data, new_data, changed_by)
                    VALUES (TG_TABLE_NAME, OLD.id::TEXT, 'DELETE', to_jsonb(OLD), NULL, _user_id);
                END IF;

                RETURN NULL;
            END;
            $$ LANGUAGE plpgsql;
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
            CREATE OR REPLACE FUNCTION audit_log_trigger_pings_fn()
            RETURNS TRIGGER AS $$
            DECLARE
                _user_id INTEGER;
            BEGIN
                BEGIN
                    _user_id := current_setting('app.audit_user', true)::integer;
                EXCEPTION WHEN OTHERS THEN
                    _user_id := NULL;
                END;

                IF _user_id IS NULL THEN
                    SELECT id INTO _user_id FROM users WHERE subject = 'system';
                END IF;

                IF (TG_OP = 'INSERT') THEN
                    INSERT INTO audit_log (table_name, record_id, operation, old_data, new_data, changed_by)
                    VALUES (TG_TABLE_NAME, NEW.device_id::TEXT, 'INSERT', NULL, to_jsonb(NEW), _user_id);
                ELSIF (TG_OP = 'UPDATE') THEN
                    INSERT INTO audit_log (table_name, record_id, operation, old_data, new_data, changed_by)
                    VALUES (TG_TABLE_NAME, NEW.device_id::TEXT, 'UPDATE', to_jsonb(OLD), to_jsonb(NEW), _user_id);
                ELSIF (TG_OP = 'DELETE') THEN
                    INSERT INTO audit_log (table_name, record_id, operation, old_data, new_data, changed_by)
                    VALUES (TG_TABLE_NAME, OLD.device_id::TEXT, 'DELETE', to_jsonb(OLD), NULL, _user_id);
                END IF;

                RETURN NULL;
            END;
            $$ LANGUAGE plpgsql;
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
            CREATE OR REPLACE FUNCTION audit_log_trigger_pings_fn()
            RETURNS TRIGGER AS $$
            DECLARE
                _user_id INTEGER;
            BEGIN
                BEGIN
                    _user_id := current_setting('app.audit_user', true)::integer;
                EXCEPTION WHEN OTHERS THEN
                    SELECT id INTO _user_id FROM users WHERE subject = 'system';
                END;

                IF (TG_OP = 'INSERT') THEN
                    INSERT INTO audit_log (table_name, record_id, operation, old_data, new_data, changed_by)
                    VALUES (TG_TABLE_NAME, NEW.device_id::TEXT, 'INSERT', NULL, to_jsonb(NEW), _user_id);
                ELSIF (TG_OP = 'UPDATE') THEN
                    INSERT INTO audit_log (table_name, record_id, operation, old_data, new_data, changed_by)
                    VALUES (TG_TABLE_NAME, NEW.device_id::TEXT, 'UPDATE', to_jsonb(OLD), to_jsonb(NEW), _user_id);
                ELSIF (TG_OP = 'DELETE') THEN
                    INSERT INTO audit_log (table_name, record_id, operation, old_data, new_data, changed_by)
                    VALUES (TG_TABLE_NAME, OLD.device_id::TEXT, 'DELETE', to_jsonb(OLD), NULL, _user_id);
                END IF;

                RETURN NULL;
            END;
            $$ LANGUAGE plpgsql;
            "#,
        )
        .await?;

        for table in ID_PK_TABLES {
            let trigger_name = format!("audit_{}", table);
            db.execute_unprepared(&format!("DROP TRIGGER IF EXISTS {trigger_name} ON {table}"))
                .await?;
            db.execute_unprepared(&format!(
                "CREATE TRIGGER {trigger_name}
                     AFTER INSERT OR UPDATE OR DELETE ON {table}
                     FOR EACH ROW EXECUTE FUNCTION audit_log_trigger_fn()"
            ))
            .await?;
        }

        db.execute_unprepared("DROP TRIGGER IF EXISTS audit_pings ON pings")
            .await?;
        db.execute_unprepared(
            "CREATE TRIGGER audit_pings
                 AFTER INSERT OR UPDATE OR DELETE ON pings
                 FOR EACH ROW EXECUTE FUNCTION audit_log_trigger_pings_fn()",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == sea_orm::DbBackend::Postgres {
            let db = manager.get_connection();

            for table in ID_PK_TABLES {
                let trigger_name = format!("audit_{}", table);
                db.execute_unprepared(&format!("DROP TRIGGER IF EXISTS {trigger_name} ON {table}"))
                    .await?;
            }

            db.execute_unprepared("DROP TRIGGER IF EXISTS audit_pings ON pings")
                .await?;

            db.execute_unprepared("DROP FUNCTION IF EXISTS audit_log_trigger_fn()")
                .await?;

            db.execute_unprepared("DROP FUNCTION IF EXISTS audit_log_trigger_pings_fn()")
                .await?;
        }

        manager
            .drop_table(Table::drop().table(AuditLog::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AuditLog {
    Table,
}
