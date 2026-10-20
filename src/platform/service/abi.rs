//! The service control manager's ABI: the service entry and its control handler, and creating,
//! configuring, starting, stopping and deleting the service (install and uninstall only).

use crate::platform::abi::{Bool, Raw};

pub const SERVICE_WIN32_OWN_PROCESS: u32 = 0x10;
pub const SERVICE_AUTO_START: u32 = 0x2;
pub const SERVICE_ERROR_NORMAL: u32 = 0x1;

pub const SERVICE_STOPPED: u32 = 1;
pub const SERVICE_START_PENDING: u32 = 2;
pub const SERVICE_STOP_PENDING: u32 = 3;
pub const SERVICE_RUNNING: u32 = 4;

pub const SERVICE_ACCEPT_STOP: u32 = 0x1;
pub const SERVICE_ACCEPT_SHUTDOWN: u32 = 0x4;
pub const SERVICE_ACCEPT_POWEREVENT: u32 = 0x40;

pub const SERVICE_CONTROL_STOP: u32 = 1;
pub const SERVICE_CONTROL_INTERROGATE: u32 = 4;
pub const SERVICE_CONTROL_SHUTDOWN: u32 = 5;
pub const SERVICE_CONTROL_POWEREVENT: u32 = 13;
pub const PBT_APMRESUMESUSPEND: u32 = 0x7;
pub const PBT_APMRESUMEAUTOMATIC: u32 = 0x12;

pub const SC_MANAGER_CONNECT: u32 = 0x1;
pub const SC_MANAGER_CREATE_SERVICE: u32 = 0x2;
pub const SERVICE_ALL_ACCESS: u32 = 0xf_01ff;

pub const SERVICE_CONFIG_DESCRIPTION: u32 = 1;
pub const SERVICE_CONFIG_FAILURE_ACTIONS: u32 = 2;
pub const SERVICE_CONFIG_FAILURE_ACTIONS_FLAG: u32 = 4;
pub const SERVICE_CONFIG_SERVICE_SID_INFO: u32 = 5;
pub const SERVICE_SID_TYPE_UNRESTRICTED: u32 = 1;
pub const SC_ACTION_RESTART: u32 = 1;

pub const NO_ERROR: u32 = 0;
pub const ERROR_CALL_NOT_IMPLEMENTED: u32 = 120;
pub const ERROR_SERVICE_SPECIFIC_ERROR: u32 = 1066;
pub const ERROR_SERVICE_DOES_NOT_EXIST: i32 = 1060;
pub const ERROR_SERVICE_NOT_ACTIVE: i32 = 1062;

/// C `SERVICE_STATUS`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ServiceStatus {
    pub service_type: u32,
    pub current_state: u32,
    pub controls_accepted: u32,
    pub win32_exit_code: u32,
    pub service_exit_code: u32,
    pub check_point: u32,
    pub wait_hint: u32,
}

pub type ServiceMain = unsafe extern "system" fn(argc: u32, argv: *mut *mut u16);
pub type HandlerEx =
    unsafe extern "system" fn(control: u32, event_type: u32, event_data: Raw, context: Raw) -> u32;

/// C `SERVICE_TABLE_ENTRYW`.
#[repr(C)]
pub struct ServiceTableEntry {
    pub name: *const u16,
    pub main: Option<ServiceMain>,
}

/// C `SC_ACTION`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ScAction {
    pub kind: u32,
    pub delay_ms: u32,
}

/// C `SERVICE_FAILURE_ACTIONSW`.
#[repr(C)]
pub struct ServiceFailureActions {
    pub reset_period_secs: u32,
    pub reboot_message: *const u16,
    pub command: *const u16,
    pub actions_count: u32,
    pub actions: *const ScAction,
}

/// C `SERVICE_DESCRIPTIONW`.
#[repr(C)]
pub struct ServiceDescription {
    pub description: *const u16,
}

/// C `SERVICE_SID_INFO`.
#[repr(C)]
pub struct ServiceSidInfo {
    pub sid_type: u32,
}

/// C `SERVICE_FAILURE_ACTIONS_FLAG`.
#[repr(C)]
pub struct ServiceFailureActionsFlag {
    pub on_non_crash_failures: Bool,
}

#[link(name = "advapi32")]
unsafe extern "system" {
    pub fn StartServiceCtrlDispatcherW(table: *const ServiceTableEntry) -> Bool;
    pub fn RegisterServiceCtrlHandlerExW(name: *const u16, handler: HandlerEx, context: Raw)
    -> Raw;
    pub fn SetServiceStatus(handle: Raw, status: *const ServiceStatus) -> Bool;
    pub fn OpenSCManagerW(machine: *const u16, database: *const u16, access: u32) -> Raw;
    pub fn CreateServiceW(
        manager: Raw,
        name: *const u16,
        display_name: *const u16,
        access: u32,
        service_type: u32,
        start_type: u32,
        error_control: u32,
        binary_path: *const u16,
        load_order_group: *const u16,
        tag_id: *mut u32,
        dependencies: *const u16,
        account: *const u16,
        password: *const u16,
    ) -> Raw;
    pub fn OpenServiceW(manager: Raw, name: *const u16, access: u32) -> Raw;
    pub fn ChangeServiceConfigW(
        service: Raw,
        service_type: u32,
        start_type: u32,
        error_control: u32,
        binary_path: *const u16,
        load_order_group: *const u16,
        tag_id: *mut u32,
        dependencies: *const u16,
        account: *const u16,
        password: *const u16,
        display_name: *const u16,
    ) -> Bool;
    pub fn ChangeServiceConfig2W(service: Raw, level: u32, info: Raw) -> Bool;
    pub fn StartServiceW(service: Raw, argc: u32, argv: *const *const u16) -> Bool;
    pub fn ControlService(service: Raw, control: u32, status: *mut ServiceStatus) -> Bool;
    pub fn QueryServiceStatus(service: Raw, status: *mut ServiceStatus) -> Bool;
    pub fn DeleteService(service: Raw) -> Bool;
    pub fn CloseServiceHandle(handle: Raw) -> Bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn layouts_match_the_sdk() {
        assert_eq!(size_of::<ServiceStatus>(), 28); // SERVICE_STATUS
        assert_eq!(offset_of!(ServiceStatus, win32_exit_code), 12); // SERVICE_STATUS.dwWin32ExitCode
        assert_eq!(offset_of!(ServiceStatus, wait_hint), 24); // SERVICE_STATUS.dwWaitHint
        assert_eq!(size_of::<ServiceTableEntry>(), 16); // SERVICE_TABLE_ENTRYW
        assert_eq!(offset_of!(ServiceTableEntry, main), 8); // SERVICE_TABLE_ENTRYW.lpServiceProc
        assert_eq!(size_of::<ScAction>(), 8); // SC_ACTION
        assert_eq!(offset_of!(ScAction, delay_ms), 4); // SC_ACTION.Delay
        assert_eq!(size_of::<ServiceFailureActions>(), 40); // SERVICE_FAILURE_ACTIONSW
        assert_eq!(offset_of!(ServiceFailureActions, reboot_message), 8); // SERVICE_FAILURE_ACTIONSW.lpRebootMsg
        assert_eq!(offset_of!(ServiceFailureActions, actions_count), 24); // SERVICE_FAILURE_ACTIONSW.cActions
        assert_eq!(offset_of!(ServiceFailureActions, actions), 32); // SERVICE_FAILURE_ACTIONSW.lpsaActions
        assert_eq!(size_of::<ServiceDescription>(), 8); // SERVICE_DESCRIPTIONW
        assert_eq!(size_of::<ServiceSidInfo>(), 4); // SERVICE_SID_INFO
        assert_eq!(size_of::<ServiceFailureActionsFlag>(), 4); // SERVICE_FAILURE_ACTIONS_FLAG
    }
}
