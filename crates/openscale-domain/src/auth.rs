use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Viewer,
    Operator,
    Admin,
    Technician,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    ViewScales,
    ViewCatalog,
    ViewReports,
    EditPlu,
    ChangePrice,
    ImportExportCatalog,
    RunSync,
    ManageMasterTables,
    ManageScales,
    ManageUsersAndKeys,
    ServiceConsole,
    ScaleSettings,
    NetworkConfig,
    RemoteControl,
    RebootScale,
}

impl Role {
    pub fn has_permission(self, permission: Permission) -> bool {
        match self {
            Role::Viewer => matches!(
                permission,
                Permission::ViewScales | Permission::ViewCatalog | Permission::ViewReports
            ),
            Role::Operator => {
                Role::Viewer.has_permission(permission)
                    || matches!(
                        permission,
                        Permission::EditPlu
                            | Permission::ChangePrice
                            | Permission::ImportExportCatalog
                            | Permission::RunSync
                    )
            }
            Role::Admin => {
                Role::Operator.has_permission(permission)
                    || matches!(
                        permission,
                        Permission::ManageMasterTables
                            | Permission::ManageScales
                            | Permission::ManageUsersAndKeys
                    )
            }
            Role::Technician => {
                Role::Viewer.has_permission(permission)
                    || matches!(
                        permission,
                        Permission::ServiceConsole
                            | Permission::ScaleSettings
                            | Permission::NetworkConfig
                            | Permission::RemoteControl
                            | Permission::RebootScale
                    )
            }
        }
    }

    pub fn require(self, permission: Permission) -> Result<(), DomainError> {
        if self.has_permission(permission) {
            Ok(())
        } else {
            Err(DomainError::PermissionDenied {
                role: format!("{self:?}"),
                permission: format!("{permission:?}"),
            })
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    /// Local desktop single-user mode (disabled when API is exposed on network).
    #[default]
    SingleUserLocal,
    /// Full RBAC authentication (JWT or API Key).
    Authenticated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub store_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub role: Role,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub store_id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub role: Role,
    pub scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rbac_matrix_enforces_least_privilege() {
        assert!(Role::Viewer.has_permission(Permission::ViewCatalog));
        assert!(!Role::Viewer.has_permission(Permission::EditPlu));

        assert!(Role::Operator.has_permission(Permission::ChangePrice));
        assert!(!Role::Operator.has_permission(Permission::ManageScales));

        assert!(Role::Admin.has_permission(Permission::ManageUsersAndKeys));
        assert!(!Role::Admin.has_permission(Permission::ServiceConsole));

        assert!(Role::Technician.has_permission(Permission::ServiceConsole));
        assert!(Role::Technician.has_permission(Permission::RebootScale));
        assert!(!Role::Technician.has_permission(Permission::ManageUsersAndKeys));

        assert!(Role::Admin.require(Permission::ManageScales).is_ok());
        assert!(matches!(
            Role::Viewer.require(Permission::EditPlu),
            Err(DomainError::PermissionDenied { .. })
        ));
    }
}
