use crate::error::Result;
use serde_json::{json, Value};

pub fn run(connection: &postgres_compat::Connection, initial_admin_password: &str) -> Result<()> {
    let initialized: bool = connection.query_row(
        "SELECT to_regclass('schema_migrations') IS NOT NULL",
        [],
        |row| row.get(0),
    )?;
    if !initialized {
        connection.execute_batch(include_str!("postgres_schema.sql"))?;
    }
    let sample_workload_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='2.3.4-sample-workload')",
        [],
        |row| row.get(0),
    )?;
    if !sample_workload_migrated {
        connection.execute_batch(include_str!("postgres_v2_3_4_sample_workload.sql"))?;
    }
    let seeded: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.0-beta.2-server-seed')",
        [],
        |row| row.get(0),
    )?;
    if !seeded {
        connection.execute_batch(include_str!("postgres_seed.sql"))?;
        connection.execute(
            "INSERT INTO schema_migrations(version) VALUES ('1.1.0-beta.2-server-seed') ON CONFLICT DO NOTHING",
            [],
        )?;
    }
    let notifications_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.0-beta.10-notifications')",
        [],
        |row| row.get(0),
    )?;
    if !notifications_migrated {
        connection.execute_batch(include_str!("postgres_notification_migration.sql"))?;
    }
    let form_config_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.1-form-config-and-rd-custom-fields')",
        [],
        |row| row.get(0),
    )?;
    if !form_config_migrated {
        connection.execute_batch(include_str!("postgres_v1_1_1_migration.sql"))?;
    }
    let beta_15_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.5-beta.1-sample-rules-drafts')",
        [],
        |row| row.get(0),
    )?;
    if !beta_15_migrated {
        connection.execute_batch(include_str!("postgres_v115_beta1_migration.sql"))?;
    }
    let draft_attachments_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.5-beta.1-draft-attachments')",
        [],
        |row| row.get(0),
    )?;
    if !draft_attachments_migrated {
        connection.execute_batch(include_str!(
            "postgres_v115_beta1_draft_attachments_migration.sql"
        ))?;
    }
    let beta_2_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.5-beta.2-notification-display-and-record-columns')",
        [],
        |row| row.get(0),
    )?;
    if !beta_2_migrated {
        connection.execute_batch(include_str!("postgres_v115_beta2_migration.sql"))?;
        migrate_legacy_notification_templates(connection)?;
        ensure_notification_fields(connection)?;
    }
    let beta_3_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.5-beta.3-portal-visibility-common-methods')",
        [],
        |row| row.get(0),
    )?;
    if !beta_3_migrated {
        connection.execute_batch(include_str!("postgres_v115_beta3_migration.sql"))?;
    }
    let beta_6_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.5-beta.6-sample-type-fields-and-notifications')",
        [],
        |row| row.get(0),
    )?;
    if !beta_6_migrated {
        connection.execute_batch(include_str!("postgres_v115_beta6_migration.sql"))?;
    }
    let beta_7_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.5-beta.7-method-code-index-compatibility')",
        [],
        |row| row.get(0),
    )?;
    if !beta_7_migrated {
        connection.execute_batch(include_str!("postgres_v115_beta7_migration.sql"))?;
    }
    let beta_8_return_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.5-beta.8-rd-return-workflow')",
        [],
        |row| row.get(0),
    )?;
    if !beta_8_return_migrated {
        connection.execute_batch(include_str!("postgres_v115_beta8_rd_return_migration.sql"))?;
    }
    let final_rd_workflow_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.5-final-rd-sender-and-return-workflow')",
        [],
        |row| row.get(0),
    )?;
    if !final_rd_workflow_migrated {
        connection.execute_batch(include_str!("postgres_v1_1_5_final_rd_workflow.sql"))?;
    }
    let notification_dedup_title_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-beta.1-notification-dedup-and-title')",
        [],
        |row| row.get(0),
    )?;
    if !notification_dedup_title_migrated {
        connection.execute_batch(include_str!("postgres_v1_1_6_notification_dedup_title.sql"))?;
    }
    let beta_2_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-beta.2-public-account-rd-permissions-sessions')",
        [],
        |row| row.get(0),
    )?;
    if !beta_2_migrated {
        connection.execute_batch(include_str!("postgres_v1_1_6_beta2_roles_sessions.sql"))?;
    }
    let beta_2_completion_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-beta.2-public-account-workflow-completion')",
        [],
        |row| row.get(0),
    )?;
    if !beta_2_completion_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_6_beta2_workflow_completion.sql"
        ))?;
    }
    let beta_4_rd_columns_void_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-beta.4-rd-columns-and-return-void')",
        [],
        |row| row.get(0),
    )?;
    if !beta_4_rd_columns_void_migrated {
        connection.execute_batch(include_str!("postgres_v1_1_6_beta4_rd_columns_void.sql"))?;
    }
    let beta_6_role_scopes_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-beta.6-role-scopes-notification-multi-groups')",
        [],
        |row| row.get(0),
    )?;
    if !beta_6_role_scopes_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_6_beta6_role_scopes_notification_groups.sql"
        ))?;
    }
    let beta_7_notification_targets_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-beta.7-notification-targets')",
        [],
        |row| row.get(0),
    )?;
    if !beta_7_notification_targets_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_6_beta7_notification_targets.sql"
        ))?;
    }
    let beta_10_option_details_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-beta.10-rd-option-detail-rules')",
        [],
        |row| row.get(0),
    )?;
    if !beta_10_option_details_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_6_beta10_option_detail_rules.sql"
        ))?;
    }
    let hotfix_4_user_affiliations_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-hotfix.4-user-multi-affiliations')",
        [],
        |row| row.get(0),
    )?;
    if !hotfix_4_user_affiliations_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_6_hotfix4_user_affiliations.sql"
        ))?;
    }
    let hotfix_6_work_scopes_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-hotfix.6-work-division-scopes')",
        [],
        |row| row.get(0),
    )?;
    if !hotfix_6_work_scopes_migrated {
        connection.execute_batch(include_str!("postgres_v1_1_6_hotfix6_work_scopes.sql"))?;
    }
    let hotfix_6_common_method_scopes_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-hotfix.6-common-method-division-scopes')",
        [],
        |row| row.get(0),
    )?;
    if !hotfix_6_common_method_scopes_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_6_hotfix6_common_method_scopes.sql"
        ))?;
    }
    let hotfix_8_trash_active_index_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-hotfix.8-trash-active-index')",
        [],
        |row| row.get(0),
    )?;
    if !hotfix_8_trash_active_index_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_6_hotfix8_trash_active_index.sql"
        ))?;
    }
    let hotfix_9_rd_rejection_notifications_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-hotfix.9-rd-rejection-group-notifications')",
        [],
        |row| row.get(0),
    )?;
    if !hotfix_9_rd_rejection_notifications_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_6_hotfix9_rd_rejection_notifications.sql"
        ))?;
    }
    let hotfix_14_sample_sequence_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-hotfix.14-sample-info-sequence')",
        [],
        |row| row.get(0),
    )?;
    if !hotfix_14_sample_sequence_migrated {
        connection.execute_batch(include_str!("postgres_v1_1_6_hotfix14_sample_sequence.sql"))?;
        connection.execute(
            "INSERT INTO schema_migrations(version) VALUES ('1.1.6-hotfix.14-sample-info-sequence') ON CONFLICT DO NOTHING",
            [],
        )?;
    }
    let hotfix_16_rd_resubmission_notifications_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-hotfix.16-rd-resubmission-notifications')",
        [],
        |row| row.get(0),
    )?;
    if !hotfix_16_rd_resubmission_notifications_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_6_hotfix16_rd_resubmission_notifications.sql"
        ))?;
    }
    let hotfix_18_sheet1_type_columns_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-hotfix.18-sheet1-type-columns')",
        [],
        |row| row.get(0),
    )?;
    if !hotfix_18_sheet1_type_columns_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_6_hotfix18_sheet1_type_columns.sql"
        ))?;
    }
    let hotfix_19_analysis_public_account_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-hotfix.19-analysis-public-account')",
        [],
        |row| row.get(0),
    )?;
    if !hotfix_19_analysis_public_account_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_6_hotfix19_analysis_public_account.sql"
        ))?;
    }
    let hotfix_20_rd_sample_withdraw_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.6-hotfix.20-rd-sample-withdraw')",
        [],
        |row| row.get(0),
    )?;
    if !hotfix_20_rd_sample_withdraw_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_6_hotfix20_rd_sample_withdraw.sql"
        ))?;
    }
    let beta_1_sample_info_return_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.7-beta.1-sample-info-return-workflow')",
        [],
        |row| row.get(0),
    )?;
    if !beta_1_sample_info_return_migrated {
        connection.execute_batch(include_str!("postgres_v1_1_7_beta1_sample_info_return.sql"))?;
    }
    let beta_2_return_visibility_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.7-beta.2-sample-info-return-visibility')",
        [],
        |row| row.get(0),
    )?;
    if !beta_2_return_visibility_migrated {
        connection.execute_batch(include_str!("postgres_v1_1_7_beta2_return_visibility.sql"))?;
    }
    let beta_3_dynamic_return_edit_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.7-beta.3-dynamic-return-edit')",
        [],
        |row| row.get(0),
    )?;
    if !beta_3_dynamic_return_edit_migrated {
        connection.execute(
            "INSERT INTO schema_migrations(version) VALUES ('1.1.7-beta.3-dynamic-return-edit') ON CONFLICT DO NOTHING",
            [],
        )?;
    }
    let beta_5_analysis_public_account_cards_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.7-beta.5-analysis-public-account-cards')",
        [],
        |row| row.get(0),
    )?;
    if !beta_5_analysis_public_account_cards_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_7_beta5_analysis_public_account_cards.sql"
        ))?;
    }
    let beta_5_analysis_public_account_home_cards_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.7-beta.5-analysis-public-account-home-cards')",
        [],
        |row| row.get(0),
    )?;
    if !beta_5_analysis_public_account_home_cards_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_7_beta5_analysis_public_account_home_cards.sql"
        ))?;
    }
    let beta_6_analysis_public_account_business_scope_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.7-beta.6-analysis-public-account-business-scope')",
        [],
        |row| row.get(0),
    )?;
    if !beta_6_analysis_public_account_business_scope_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_7_beta6_analysis_public_account_business_scope.sql"
        ))?;
    }
    let beta_8_sample_info_return_permissions_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.7-beta.8-sample-info-return-permissions')",
        [],
        |row| row.get(0),
    )?;
    if !beta_8_sample_info_return_permissions_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_1_7_beta8_sample_info_return_permissions.sql"
        ))?;
    }
    let alpha_1_role_templates_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.2.0-alpha.1-role-templates')",
        [],
        |row| row.get(0),
    )?;
    if !alpha_1_role_templates_migrated {
        connection.execute_batch(include_str!("postgres_v1_2_0_alpha1_role_templates.sql"))?;
    }
    let alpha_2_user_departments_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.2.0-alpha.2-user-primary-and-business-departments')",
        [],
        |row| row.get(0),
    )?;
    if !alpha_2_user_departments_migrated {
        connection.execute_batch(include_str!("postgres_v1_2_0_alpha2_user_departments.sql"))?;
    }
    let alpha_3_record_ownership_snapshots_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.2.0-alpha.3-record-ownership-snapshots')",
        [],
        |row| row.get(0),
    )?;
    if !alpha_3_record_ownership_snapshots_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_2_0_alpha3_record_ownership_snapshots.sql"
        ))?;
    }
    let alpha_4_organization_governance_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.2.0-alpha.4-organization-governance')",
        [],
        |row| row.get(0),
    )?;
    if !alpha_4_organization_governance_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_2_0_alpha4_organization_governance.sql"
        ))?;
    }
    let alpha_4_department_role_assignment_sources_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.2.0-alpha.4-department-role-assignment-sources')",
        [],
        |row| row.get(0),
    )?;
    if !alpha_4_department_role_assignment_sources_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_2_0_alpha4_department_role_assignment_sources.sql"
        ))?;
    }
    let alpha_5_analysis_export_dimensions_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.2.0-alpha.5-analysis-export-dimensions')",
        [],
        |row| row.get(0),
    )?;
    if !alpha_5_analysis_export_dimensions_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_2_0_alpha5_analysis_export_dimensions.sql"
        ))?;
    }
    let alpha_6_scoped_workload_export_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.2.0-alpha.6-scoped-workload-export')",
        [],
        |row| row.get(0),
    )?;
    if !alpha_6_scoped_workload_export_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_2_0_alpha6_scoped_workload_export.sql"
        ))?;
    }
    let alpha_7_ownership_confirmation_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.2.0-alpha.7-ownership-confirmation-snapshots')",
        [],
        |row| row.get(0),
    )?;
    if !alpha_7_ownership_confirmation_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_2_0_alpha7_ownership_confirmation.sql"
        ))?;
    }
    let alpha_8_strict_role_scopes_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.2.0-alpha.8-strict-role-data-scopes')",
        [],
        |row| row.get(0),
    )?;
    if !alpha_8_strict_role_scopes_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_2_0_alpha8_strict_role_scopes.sql"
        ))?;
    }
    let alpha_10_known_fixes_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.2.0-alpha.10-known-fixes')",
        [],
        |row| row.get(0),
    )?;
    if !alpha_10_known_fixes_migrated {
        connection.execute_batch(include_str!("postgres_v1_2_0_alpha10_known_fixes.sql"))?;
    }
    let alpha_10_method_type_fix_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.2.0-alpha.10-method-type-single')",
        [],
        |row| row.get(0),
    )?;
    if !alpha_10_method_type_fix_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_2_0_alpha10_method_type_single.sql"
        ))?;
    }
    let alpha_11_login_persistence_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.2.0-alpha.11-login-persistence')",
        [],
        |row| row.get(0),
    )?;
    if !alpha_11_login_persistence_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_2_0_alpha11_login_persistence.sql"
        ))?;
    }
    let alpha_12_sending_analysis_permissions_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.2.0-alpha.12-sending-analysis-permissions')",
        [],
        |row| row.get(0),
    )?;
    if !alpha_12_sending_analysis_permissions_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_2_0_alpha12_sending_analysis_permissions.sql"
        ))?;
    }
    let personnel_feedback_notification_target_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.3.2-personnel-feedback-notification-target')",
        [],
        |row| row.get(0),
    )?;
    if !personnel_feedback_notification_target_migrated {
        connection.execute_batch(include_str!(
            "postgres_v1_3_2_personnel_feedback_notification_target.sql"
        ))?;
    }
    let personnel_feedback_rejection_notification_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='2.1.0-personnel-feedback-rejection-notification')",
        [],
        |row| row.get(0),
    )?;
    if !personnel_feedback_rejection_notification_migrated {
        connection.execute_batch(include_str!(
            "postgres_v2_1_0_personnel_feedback_rejection_notification.sql"
        ))?;
    }
    let help_tutorial_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='2.2.0-help-tutorial')",
        [],
        |row| row.get(0),
    )?;
    if !help_tutorial_migrated {
        connection.execute_batch(include_str!("postgres_v2_2_0_help_tutorial.sql"))?;
    }
    let sample_info_editor_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='2.2.4-sample-info-editor')",
        [],
        |row| row.get(0),
    )?;
    if !sample_info_editor_migrated {
        connection.execute_batch(include_str!("postgres_v2_2_4_sample_info_editor.sql"))?;
    }
    let v2_2_8_performance_indexes_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='2.2.8-performance-indexes')",
        [],
        |row| row.get(0),
    )?;
    if !v2_2_8_performance_indexes_migrated {
        connection.execute_batch(include_str!("postgres_v2_2_8_performance_indexes.sql"))?;
    }
    let v2_2_11_export_template_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='2.2.11-workload-export-template')",
        [],
        |row| row.get(0),
    )?;
    if !v2_2_11_export_template_migrated {
        migrate_v2_2_11_export_template(connection)?;
    }
    // A previous 2.2.11 build could already have recorded the migration marker
    // before the runtime writer/template alignment was complete.  Run the
    // corrective pass under a separate marker so upgraded databases are fixed
    // without touching the 2.2.10 baseline or administrator custom templates.
    let v2_2_11_export_template_v2_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='2.2.11-workload-export-template-v2')",
        [],
        |row| row.get(0),
    )?;
    if !v2_2_11_export_template_v2_migrated {
        migrate_v2_2_11_export_template_v2(connection)?;
    }
    let v2_2_12_workload_export_template_locked: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='2.2.12-workload-export-template-lock')",
        [],
        |row| row.get(0),
    )?;
    if !v2_2_12_workload_export_template_locked {
        migrate_v2_2_12_workload_export_template_lock(connection)?;
    }
    let v2_2_16_detection_type_visibility_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='2.2.16-detection-type-visibility')",
        [],
        |row| row.get(0),
    )?;
    if !v2_2_16_detection_type_visibility_migrated {
        connection.execute_batch(include_str!(
            "postgres_v2_2_16_detection_type_visibility.sql"
        ))?;
    }
    let v2_2_17_auxiliary_work_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='2.2.17-auxiliary-work')",
        [],
        |row| row.get(0),
    )?;
    if !v2_2_17_auxiliary_work_migrated {
        connection.execute_batch(include_str!("postgres_v2_2_17_auxiliary_work.sql"))?;
        connection.execute("INSERT INTO schema_migrations(version) VALUES('2.2.17-auxiliary-work') ON CONFLICT DO NOTHING", [])?;
    }
    connection.execute_batch("INSERT INTO role_permissions(role_id,permission_key) SELECT r.id,'manage:notifications' FROM roles r WHERE r.name IN ('系统管理员','分析检测组长') AND NOT EXISTS(SELECT 1 FROM role_permissions rp WHERE rp.role_id=r.id AND rp.permission_key='manage:notifications');")?;
    if !initialized {
        let password_hash = bcrypt::hash(initial_admin_password, bcrypt::DEFAULT_COST)
            .map_err(|error| crate::error::AppError::Internal(error.to_string()))?;
        connection.execute(
            "UPDATE users SET password=?1, updated_at=datetime('now','localtime') WHERE username='admin'",
            postgres_compat::params![password_hash],
        )?;
    }
    Ok(())
}

/// Upgrade the shipped workload export template while preserving administrator customisations.
/// Older installs used the generic "人员汇总表" sheet definition, which caused the runtime
/// writer's personnel/workload layout to be overwritten on every export.
fn migrate_v2_2_11_export_template(connection: &postgres_compat::Connection) -> Result<()> {
    let raw = connection
        .query_row(
            "SELECT value FROM system_settings WHERE key='export_template_workload'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok();
    if let Some(raw) = raw {
        if let Ok(mut config) = serde_json::from_str::<Value>(&raw) {
            if let Some(sheets) = config.get_mut("sheets").and_then(Value::as_object_mut) {
                if let Some(sheet6) = sheets.get_mut("sheet6").and_then(Value::as_object_mut) {
                    let old_title = sheet6
                        .get("title")
                        .and_then(Value::as_str)
                        .map(|title| title == "人员汇总表")
                        .unwrap_or(false);
                    if old_title {
                        sheet6.insert("title".into(), Value::String("人员工作量汇总".into()));
                    }
                    let legacy_columns = sheet6
                        .get("columns")
                        .and_then(Value::as_object)
                        .map(|columns| {
                            columns.len() == 4
                                && columns.contains_key("user_name")
                                && columns.contains_key("quantity")
                                && columns.contains_key("count")
                                && columns.contains_key("instruments")
                        })
                        .unwrap_or(false);
                    if legacy_columns {
                        sheet6.insert(
                            "columns".into(),
                            serde_json::json!({
                                "user_name":{"label":"项目","width":14},
                                "instrument":{"label":"仪器","width":24},
                                "method_type":{"label":"类型","width":16},
                                "method":{"label":"方法","width":30},
                                "coefficient":{"label":"系数","width":10},
                                "quantity":{"label":"数量","width":10},
                                "workload":{"label":"工作量明细","width":16},
                                "total_workload":{"label":"人员工作量汇总","width":18}
                            }),
                        );
                    }
                }
                sheets.entry("sheet12").or_insert_with(|| {
                    serde_json::json!({
                        "id":"sheet12",
                        "title":"辅助工作明细汇总",
                        "color":"#FF9800",
                        "enabled":true,
                        "columns":{
                            "user_name":{"label":"项目","width":14},
                            "auxiliary_method":{"label":"辅助工作名称","width":28},
                            "coefficient":{"label":"系数","width":10},
                            "quantity":{"label":"数量","width":10},
                            "workload":{"label":"明细工作量","width":16},
                            "total_workload":{"label":"辅助工作量汇总","width":18}
                        }
                    })
                });
            }
            connection.execute(
                "UPDATE system_settings SET value=?1,updated_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS') WHERE key='export_template_workload'",
                postgres_compat::params![config.to_string()],
            )?;
        }
    }
    connection.execute(
        "INSERT INTO schema_migrations(version) VALUES ('2.2.11-workload-export-template') ON CONFLICT DO NOTHING",
        [],
    )?;
    Ok(())
}

/// Corrective export-template pass for databases upgraded by the first 2.2.11
/// package.  The old package could leave Sheet 6 as the four-column
/// “人员汇总表”; that configuration overrides the writer and produces the
/// legacy export seen by users.
fn migrate_v2_2_11_export_template_v2(connection: &postgres_compat::Connection) -> Result<()> {
    let raw = connection
        .query_row(
            "SELECT value FROM system_settings WHERE key='export_template_workload'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok();
    if let Some(raw) = raw {
        if let Ok(mut config) = serde_json::from_str::<Value>(&raw) {
            if let Some(sheets) = config.get_mut("sheets").and_then(Value::as_object_mut) {
                if let Some(sheet6) = sheets.get_mut("sheet6").and_then(Value::as_object_mut) {
                    let legacy = sheet6
                        .get("columns")
                        .and_then(Value::as_object)
                        .map(|columns| {
                            columns.contains_key("count") || columns.contains_key("instruments")
                        })
                        .unwrap_or(true);
                    if legacy || sheet6.get("title").and_then(Value::as_str) == Some("人员汇总表")
                    {
                        sheet6.insert("title".into(), Value::String("人员工作量汇总".into()));
                        sheet6.insert(
                            "columns".into(),
                            serde_json::json!({
                                "user_name": {"label":"项目","width":14},
                                "instrument": {"label":"仪器","width":24},
                                "method_type": {"label":"类型","width":16},
                                "method": {"label":"方法","width":30},
                                "coefficient": {"label":"系数","width":10},
                                "quantity": {"label":"数量","width":10},
                                "workload": {"label":"工作量明细","width":16},
                                "total_workload": {"label":"人员工作量汇总","width":18}
                            }),
                        );
                    }
                }
                sheets.entry("sheet12").or_insert_with(|| {
                    serde_json::json!({
                        "id":"sheet12", "title":"辅助工作明细汇总",
                        "color":"#FF9800", "enabled":true,
                        "columns": {
                            "user_name": {"label":"项目","width":14},
                            "auxiliary_method": {"label":"辅助工作名称","width":28},
                            "coefficient": {"label":"系数","width":10},
                            "quantity": {"label":"数量","width":10},
                            "workload": {"label":"明细工作量","width":16},
                            "total_workload": {"label":"辅助工作量汇总","width":18}
                        }
                    })
                });
                sheets.entry("sheet13").or_insert_with(|| {
                    serde_json::json!({
                        "id":"sheet13", "title":"人员工作量汇总",
                        "color":"#3F51B5", "enabled":true,
                        "columns": {
                            "user_name": {"label":"检测人","width":16},
                            "total_workload": {"label":"总工作量","width":14},
                            "method_type": {"label":"检测类型","width":18},
                            "coefficient": {"label":"系数","width":10},
                            "quantity": {"label":"数量","width":10},
                            "workload": {"label":"工作量","width":14}
                        }
                    })
                });
            }
            connection.execute(
                "UPDATE system_settings SET value=?1,updated_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS') WHERE key='export_template_workload'",
                postgres_compat::params![config.to_string()],
            )?;
        }
    }
    connection.execute(
        "INSERT INTO schema_migrations(version) VALUES ('2.2.11-workload-export-template-v2') ON CONFLICT DO NOTHING",
        [],
    )?;
    Ok(())
}

/// Normalize the three workload sheets on every upgraded database. Their
/// physical schemas are owned by the export writer; old template columns and
/// labels must not be able to reintroduce the legacy layout.
fn migrate_v2_2_12_workload_export_template_lock(
    connection: &postgres_compat::Connection,
) -> Result<()> {
    let raw = connection
        .query_row(
            "SELECT value FROM system_settings WHERE key='export_template_workload'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok();
    if let Some(raw) = raw {
        if let Ok(mut config) = serde_json::from_str::<Value>(&raw) {
            if let Some(sheets) = config.get_mut("sheets").and_then(Value::as_object_mut) {
                let definitions = [
                    (
                        "sheet6",
                        "人员工作量明细",
                        "#00BCD4",
                        serde_json::json!({
                            "user_name": {"label":"项目","width":14},
                            "instrument": {"label":"仪器","width":24},
                            "method_type": {"label":"类型","width":16},
                            "method": {"label":"方法","width":30},
                            "coefficient": {"label":"系数","width":10},
                            "quantity": {"label":"数量","width":10},
                            "workload": {"label":"工作量明细","width":16},
                            "total_workload": {"label":"人员工作量汇总","width":18}
                        }),
                    ),
                    (
                        "sheet12",
                        "辅助工作明细汇总",
                        "#FF9800",
                        serde_json::json!({
                            "user_name": {"label":"项目","width":14},
                            "auxiliary_method": {"label":"辅助工作名称","width":28},
                            "coefficient": {"label":"系数","width":10},
                            "quantity": {"label":"数量","width":10},
                            "workload": {"label":"明细工作量","width":16},
                            "total_workload": {"label":"辅助工作量汇总","width":18}
                        }),
                    ),
                    (
                        "sheet13",
                        "人员工作量汇总",
                        "#3F51B5",
                        serde_json::json!({
                            "user_name": {"label":"检测人","width":16},
                            "total_workload": {"label":"总工作量","width":14},
                            "method_type": {"label":"检测类型","width":18},
                            "coefficient": {"label":"系数","width":10},
                            "quantity": {"label":"数量","width":10},
                            "workload": {"label":"工作量","width":14}
                        }),
                    ),
                ];
                for (id, title, color, columns) in definitions {
                    let entry = sheets.entry(id).or_insert_with(|| serde_json::json!({}));
                    if !entry.is_object() {
                        *entry = serde_json::json!({});
                    }
                    let sheet = entry
                        .as_object_mut()
                        .expect("workload sheet config must be an object");
                    sheet.insert("id".into(), Value::String(id.into()));
                    sheet.insert("title".into(), Value::String(title.into()));
                    sheet.insert("color".into(), Value::String(color.into()));
                    if !sheet.get("enabled").is_some_and(Value::is_boolean) {
                        sheet.insert("enabled".into(), Value::Bool(true));
                    }
                    sheet.insert("columns".into(), columns);
                }
            }
            connection.execute(
                "UPDATE system_settings SET value=?1,updated_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS') WHERE key='export_template_workload'",
                postgres_compat::params![config.to_string()],
            )?;
        }
    }
    connection.execute(
        "INSERT INTO schema_migrations(version) VALUES ('2.2.12-workload-export-template-lock') ON CONFLICT DO NOTHING",
        [],
    )?;
    Ok(())
}

fn migrate_legacy_notification_templates(connection: &postgres_compat::Connection) -> Result<()> {
    for (old_key, new_key) in [
        ("notification_template_rd_work_record", "rd_work_record"),
        ("notification_template_sample_info", "sample_info"),
    ] {
        let value = connection
            .query_row(
                "SELECT value FROM system_settings WHERE key=?1",
                [old_key],
                |row| row.get::<_, String>(0),
            )
            .ok();
        let Some(value) = value else { continue };
        let Ok(config) = serde_json::from_str::<Value>(&value) else {
            continue;
        };
        let title = config.get("title").and_then(Value::as_str).unwrap_or("");
        let footer = config.get("footer").and_then(Value::as_str).unwrap_or("");
        let fields = config.get("fields").cloned().unwrap_or_else(|| json!([]));
        if title.trim().is_empty() || !fields.is_array() {
            continue;
        }
        connection.execute(
            "UPDATE notification_templates SET title=?1,footer=COALESCE(NULLIF(?2,''),footer),field_config_json=?3,is_active=1 WHERE event_key=?4",
            postgres_compat::params![title, footer, fields.to_string(), new_key],
        )?;
    }
    Ok(())
}

fn ensure_notification_fields(connection: &postgres_compat::Connection) -> Result<()> {
    let defaults = [
        ("sample_info", "main_components", "主要成分"),
        ("sample_info", "notes", "备注"),
        ("sample_info", "attachment_count", "附件数量"),
        ("sample_info", "status", "状态"),
        ("rd_work_record", "batch_no", "批号"),
        ("rd_work_record", "instrument_code", "仪器"),
        ("rd_work_record", "status", "状态"),
        ("rd_work_record", "notes", "备注"),
    ];
    for (template_key, field_key, label) in defaults {
        let Some(config) = connection
            .query_row(
                "SELECT field_config_json FROM notification_templates WHERE event_key=?1 AND is_active=1",
                [template_key],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|value| serde_json::from_str::<Value>(&value).ok()) else { continue };
        let mut fields = config.as_array().cloned().unwrap_or_default();
        if fields
            .iter()
            .any(|field| field.get("key").and_then(Value::as_str) == Some(field_key))
        {
            continue;
        }
        fields.push(json!({"key":field_key,"label":label,"visible":true,"enabled":true,"bold":false,"sort_order":fields.len()+1}));
        connection.execute(
            "UPDATE notification_templates SET field_config_json=?,updated_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS') WHERE event_key=?",
            postgres_compat::params![Value::Array(fields).to_string(), template_key],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use postgres_compat::Connection;

    #[test]
    fn replaces_legacy_full_method_code_index() {
        let conn = Connection::open_test_database().expect("PostgreSQL test database");
        run(&conn, "admin123").expect("initial migrations");
        conn.execute_batch(
            "UPDATE methods SET method_code='M-LEGACY-' || id WHERE method_code='';
             DROP INDEX IF EXISTS uq_methods_code_present;
             CREATE UNIQUE INDEX idx_methods_code ON methods(method_code);
             INSERT INTO methods(method_code,name,full_name,coefficient,multiplier,amount,is_active,notes)
             VALUES('','历史空编号方法','',1,1,0,1,'');
             DELETE FROM schema_migrations
             WHERE version='1.1.5-beta.7-method-code-index-compatibility';",
        )
        .expect("simulate legacy index");

        run(&conn, "admin123").expect("compatibility migration");

        let legacy_index_exists: bool = conn
            .query_row(
                "SELECT to_regclass('idx_methods_code') IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .expect("legacy index state");
        let replacement_index: String = conn
            .query_row(
                "SELECT indexdef FROM pg_indexes WHERE schemaname=current_schema() AND indexname='uq_methods_code_present'",
                [],
                |row| row.get(0),
            )
            .expect("replacement index");
        assert!(!legacy_index_exists);
        assert!(replacement_index.contains("WHERE (method_code <> ''::text)"));

        conn.execute(
            "INSERT INTO methods(method_code,name,full_name,coefficient,multiplier,amount,is_active,notes)
             VALUES('','第二条空编号方法','',1,1,0,1,'')",
            [],
        )
        .expect("partial index must allow multiple unnumbered legacy methods");
    }

    #[test]
    fn final_rd_workflow_migration_backfills_activity_time_and_is_idempotent() {
        let conn = Connection::open_test_database().expect("PostgreSQL test database");
        run(&conn, "admin123").expect("initial migrations");
        conn.execute_batch(
            "ALTER TABLE rd_work_records DROP COLUMN last_activity_at;
             DELETE FROM schema_migrations
             WHERE version='1.1.5-final-rd-sender-and-return-workflow';
             INSERT INTO project_groups(name) VALUES('workflow-migration-lab');
             INSERT INTO projects(group_id,name) VALUES(
               (SELECT id FROM project_groups WHERE name='workflow-migration-lab'),
               'workflow-migration-project'
             );
             INSERT INTO rd_work_records(project_id,user_name,quantity,recorded_at)
             VALUES(
               (SELECT id FROM projects WHERE name='workflow-migration-project'),
               'workflow-migration-sender',1,'2000-01-01T10:00:00'
             );",
        )
        .expect("simulate pre-1.1.5 schema");

        run(&conn, "admin123").expect("final workflow migration");
        let activity_at: String = conn
            .query_row(
                "SELECT last_activity_at FROM rd_work_records WHERE user_name='workflow-migration-sender'",
                [],
                |row| row.get(0),
            )
            .expect("backfilled activity timestamp");
        assert_eq!(activity_at, "2000-01-01T10:00:00");

        run(&conn, "admin123").expect("idempotent final workflow migration");
    }

    #[test]
    fn notification_title_migration_upgrades_only_the_legacy_default() {
        let conn = Connection::open_test_database().expect("PostgreSQL test database");
        run(&conn, "admin123").expect("initial migrations");
        conn.execute(
            "INSERT INTO notification_templates(event_key,title,footer,field_config_json,is_active)
             VALUES('rd_work_record','新研发送样记录','', '[]', 1)
             ON CONFLICT(event_key) DO UPDATE SET title=excluded.title",
            [],
        )
        .expect("legacy RD template");
        conn.execute(
            "DELETE FROM schema_migrations
             WHERE version='1.1.6-beta.1-notification-dedup-and-title'",
            [],
        )
        .expect("reset notification title migration");

        run(&conn, "admin123").expect("upgrade legacy notification title");
        let upgraded: String = conn
            .query_row(
                "SELECT title FROM notification_templates WHERE event_key='rd_work_record'",
                [],
                |row| row.get(0),
            )
            .expect("upgraded title");
        assert_eq!(upgraded, "【{{检测类型}}】新研发送样记录");

        conn.execute(
            "UPDATE notification_templates SET title='实验室自定义通知标题'
             WHERE event_key='rd_work_record'",
            [],
        )
        .expect("custom notification title");
        conn.execute(
            "DELETE FROM schema_migrations
             WHERE version='1.1.6-beta.1-notification-dedup-and-title'",
            [],
        )
        .expect("reset notification title migration again");

        run(&conn, "admin123").expect("preserve custom notification title");
        let preserved: String = conn
            .query_row(
                "SELECT title FROM notification_templates WHERE event_key='rd_work_record'",
                [],
                |row| row.get(0),
            )
            .expect("preserved title");
        assert_eq!(preserved, "实验室自定义通知标题");
    }

    #[test]
    fn option_detail_rules_migration_upgrades_legacy_select_other_field() {
        let conn = Connection::open_test_database().expect("PostgreSQL test database");
        run(&conn, "admin123").expect("initial migrations");
        conn.execute_batch(
            "UPDATE rd_record_columns
             SET data_type='select_other', option_detail_rules=''
             WHERE name='notes';
             DELETE FROM schema_migrations
             WHERE version='1.1.6-beta.10-rd-option-detail-rules';",
        )
        .expect("simulate beta.9 select_other field");

        run(&conn, "admin123").expect("option detail migration");
        let (data_type, rules): (String, String) = conn
            .query_row(
                "SELECT data_type,option_detail_rules FROM rd_record_columns WHERE name='notes'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("migrated notes field");
        assert_eq!(data_type, "select");
        assert!(rules.contains("其他"));
        assert!(rules.contains("补充说明"));
    }

    #[test]
    fn replaces_legacy_full_trash_index_with_active_only_index() {
        let conn = Connection::open_test_database().expect("PostgreSQL test database");
        run(&conn, "admin123").expect("initial migrations");
        conn.execute_batch(
            "DROP INDEX IF EXISTS idx_trash_active_entity;
             CREATE UNIQUE INDEX idx_trash_active_entity ON trash_entries(table_name,record_id);
             DELETE FROM schema_migrations
             WHERE version='1.1.6-hotfix.8-trash-active-index';",
        )
        .expect("simulate legacy trash index");

        run(&conn, "admin123").expect("trash index migration");
        let index_def: String = conn
            .query_row(
                "SELECT indexdef FROM pg_indexes
                 WHERE schemaname=current_schema() AND indexname='idx_trash_active_entity'",
                [],
                |row| row.get(0),
            )
            .expect("active trash index");
        let normalized = index_def.to_ascii_lowercase();
        assert!(normalized.contains("where"));
        assert!(normalized.contains("restored_at is null"));
        assert!(normalized.contains("purged_at is null"));

        conn.execute(
            "INSERT INTO trash_entries(entity_type,table_name,record_id,category,module,display_name,deleted_by_username)
             VALUES('实验室','project_groups',880,'master','shared','历史实验室','tester')",
            [],
        )
        .expect("historical trash entry");
        conn.execute(
            "UPDATE trash_entries SET restored_at='2026-08-05 12:00:00'
             WHERE table_name='project_groups' AND record_id=880",
            [],
        )
        .expect("restore historical entry");
        conn.execute(
            "INSERT INTO trash_entries(entity_type,table_name,record_id,category,module,display_name,deleted_by_username)
             VALUES('实验室','project_groups',880,'master','shared','再次删除实验室','tester')",
            [],
        )
        .expect("a restored entry must not block a later delete");
        let entries: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trash_entries WHERE table_name='project_groups' AND record_id=880",
                [],
                |row| row.get(0),
            )
            .expect("history count");
        assert_eq!(entries, 2);
    }

    #[test]
    fn sample_workload_migration_adds_deduplicated_source_columns() {
        if std::env::var("WORKLOAD_TEST_DATABASE_URL").is_err() {
            return;
        }
        let conn = Connection::open_test_database().expect("PostgreSQL test database");
        run(&conn, "admin123").expect("initial migrations");
        let source_columns: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM information_schema.columns
                 WHERE table_schema=current_schema() AND table_name='work_records'
                   AND column_name IN ('source_type','source_record_id')",
                [],
                |row| row.get(0),
            )
            .expect("source columns");
        assert_eq!(source_columns, 2);
        let index_exists: bool = conn
            .query_row(
                "SELECT to_regclass('uq_work_records_sample_source') IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .expect("source uniqueness index");
        assert!(index_exists);
    }
}
