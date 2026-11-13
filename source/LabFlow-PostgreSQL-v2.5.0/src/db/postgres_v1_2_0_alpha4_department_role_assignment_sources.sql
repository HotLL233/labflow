-- v1.2.0-alpha.4 follow-up: distinguish a direct user role from one that was
-- automatically added because of a department-role assignment.
CREATE TABLE IF NOT EXISTS department_role_managed_user_roles (
    user_id BIGINT NOT NULL,
    role_id BIGINT NOT NULL,
    created_at TEXT NOT NULL DEFAULT to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS'),
    PRIMARY KEY (user_id, role_id)
);

INSERT INTO schema_migrations(version)
VALUES ('1.2.0-alpha.4-department-role-assignment-sources')
ON CONFLICT DO NOTHING;
