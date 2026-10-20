/* Prints the size of every C type and the offset of every field the Rust ABI relies on, one
 * "NAME VALUE" line each, as the SDK and the vendored wireguard.h lay them out for x64.
 * tools/verify-abi.ps1 compiles it and compares every line with the Rust layout tests, whose
 * assertions carry the same NAME in a trailing comment. */
#include <stddef.h>
#include <stdio.h>
#include "wireguard.h"
#include <iphlpapi.h>
#include <shtypes.h>
#include <shellapi.h>

#define SIZE(type) printf("%s %zu\n", #type, sizeof(type))
#define OFFSET(type, field) printf("%s.%s %zu\n", #type, #field, offsetof(type, field))

int main(void)
{
    SIZE(GUID);
    OFFSET(GUID, Data2);
    OFFSET(GUID, Data3);
    OFFSET(GUID, Data4);
    SIZE(DATA_BLOB);
    OFFSET(DATA_BLOB, pbData);
    SIZE(SYSTEMTIME);
    OFFSET(SYSTEMTIME, wDay);
    OFFSET(SYSTEMTIME, wMilliseconds);

    /* src/platform/pipe/abi.rs */
    SIZE(SECURITY_ATTRIBUTES);
    OFFSET(SECURITY_ATTRIBUTES, lpSecurityDescriptor);
    OFFSET(SECURITY_ATTRIBUTES, bInheritHandle);

    /* src/platform/service/abi.rs */
    SIZE(SERVICE_STATUS);
    OFFSET(SERVICE_STATUS, dwWin32ExitCode);
    OFFSET(SERVICE_STATUS, dwWaitHint);
    SIZE(SERVICE_TABLE_ENTRYW);
    OFFSET(SERVICE_TABLE_ENTRYW, lpServiceProc);
    SIZE(SC_ACTION);
    OFFSET(SC_ACTION, Delay);
    SIZE(SERVICE_FAILURE_ACTIONSW);
    OFFSET(SERVICE_FAILURE_ACTIONSW, lpRebootMsg);
    OFFSET(SERVICE_FAILURE_ACTIONSW, cActions);
    OFFSET(SERVICE_FAILURE_ACTIONSW, lpsaActions);
    SIZE(SERVICE_DESCRIPTIONW);
    SIZE(SERVICE_SID_INFO);
    SIZE(SERVICE_FAILURE_ACTIONS_FLAG);

    /* src/platform/inet.rs */
    SIZE(SOCKADDR_INET);
    OFFSET(SOCKADDR_IN, sin_port);
    OFFSET(SOCKADDR_IN, sin_addr);

    /* src/platform/wireguard/layout.rs */
    SIZE(WIREGUARD_INTERFACE);
    OFFSET(WIREGUARD_INTERFACE, ListenPort);
    OFFSET(WIREGUARD_INTERFACE, PrivateKey);
    OFFSET(WIREGUARD_INTERFACE, PublicKey);
    OFFSET(WIREGUARD_INTERFACE, PeersCount);
    SIZE(WIREGUARD_PEER);
    OFFSET(WIREGUARD_PEER, PublicKey);
    OFFSET(WIREGUARD_PEER, PresharedKey);
    OFFSET(WIREGUARD_PEER, PersistentKeepalive);
    OFFSET(WIREGUARD_PEER, Endpoint);
    OFFSET(WIREGUARD_PEER, TxBytes);
    OFFSET(WIREGUARD_PEER, RxBytes);
    OFFSET(WIREGUARD_PEER, LastHandshake);
    OFFSET(WIREGUARD_PEER, AllowedIPsCount);
    SIZE(WIREGUARD_ALLOWED_IP);
    OFFSET(WIREGUARD_ALLOWED_IP, AddressFamily);
    OFFSET(WIREGUARD_ALLOWED_IP, Cidr);
    OFFSET(WIREGUARD_ALLOWED_IP, Flags);

    /* src/platform/ip_helper/abi.rs */
    SIZE(IP_ADDRESS_PREFIX);
    OFFSET(IP_ADDRESS_PREFIX, PrefixLength);
    SIZE(MIB_IPFORWARD_ROW2);
    OFFSET(MIB_IPFORWARD_ROW2, InterfaceIndex);
    OFFSET(MIB_IPFORWARD_ROW2, DestinationPrefix);
    OFFSET(MIB_IPFORWARD_ROW2, NextHop);
    OFFSET(MIB_IPFORWARD_ROW2, SitePrefixLength);
    OFFSET(MIB_IPFORWARD_ROW2, ValidLifetime);
    OFFSET(MIB_IPFORWARD_ROW2, PreferredLifetime);
    OFFSET(MIB_IPFORWARD_ROW2, Metric);
    OFFSET(MIB_IPFORWARD_ROW2, Protocol);
    OFFSET(MIB_IPFORWARD_ROW2, Loopback);
    OFFSET(MIB_IPFORWARD_ROW2, Immortal);
    OFFSET(MIB_IPFORWARD_ROW2, Age);
    OFFSET(MIB_IPFORWARD_ROW2, Origin);
    OFFSET(MIB_IPFORWARD_TABLE2, Table);
    OFFSET(MIB_UNICASTIPADDRESS_TABLE, Table);
    SIZE(MIB_UNICASTIPADDRESS_ROW);
    OFFSET(MIB_UNICASTIPADDRESS_ROW, InterfaceLuid);
    OFFSET(MIB_UNICASTIPADDRESS_ROW, InterfaceIndex);
    OFFSET(MIB_UNICASTIPADDRESS_ROW, PrefixOrigin);
    OFFSET(MIB_UNICASTIPADDRESS_ROW, ValidLifetime);
    OFFSET(MIB_UNICASTIPADDRESS_ROW, PreferredLifetime);
    OFFSET(MIB_UNICASTIPADDRESS_ROW, OnLinkPrefixLength);
    OFFSET(MIB_UNICASTIPADDRESS_ROW, SkipAsSource);
    OFFSET(MIB_UNICASTIPADDRESS_ROW, DadState);
    OFFSET(MIB_UNICASTIPADDRESS_ROW, ScopeId);
    OFFSET(MIB_UNICASTIPADDRESS_ROW, CreationTimeStamp);
    SIZE(MIB_IPINTERFACE_ROW);
    OFFSET(MIB_IPINTERFACE_ROW, InterfaceLuid);
    OFFSET(MIB_IPINTERFACE_ROW, InterfaceIndex);
    OFFSET(MIB_IPINTERFACE_ROW, UseAutomaticMetric);
    OFFSET(MIB_IPINTERFACE_ROW, SitePrefixLength);
    OFFSET(MIB_IPINTERFACE_ROW, Metric);
    OFFSET(MIB_IPINTERFACE_ROW, NlMtu);
    SIZE(DNS_INTERFACE_SETTINGS);
    OFFSET(DNS_INTERFACE_SETTINGS, Flags);
    OFFSET(DNS_INTERFACE_SETTINGS, NameServer);
    OFFSET(DNS_INTERFACE_SETTINGS, RegistrationEnabled);
    OFFSET(DNS_INTERFACE_SETTINGS, ProfileNameServer);

    /* src/platform/window/: the window, its clock and its file picker */
    SIZE(MSG);
    SIZE(WNDCLASSEXW);
    SIZE(WINDOWPLACEMENT);
    OFFSET(WINDOWPLACEMENT, rcNormalPosition);
    SIZE(MINMAXINFO);
    OFFSET(MINMAXINFO, ptMinTrackSize);
    SIZE(MONITORINFO);
    OFFSET(MONITORINFO, rcWork);
    SIZE(TIME_ZONE_INFORMATION);
    OFFSET(TIME_ZONE_INFORMATION, DaylightBias);
    SIZE(COMDLG_FILTERSPEC);

    /* src/platform/window/tray/abi.rs: the notification icon and its menu */
    SIZE(NOTIFYICONDATAW);
    OFFSET(NOTIFYICONDATAW, hIcon);
    OFFSET(NOTIFYICONDATAW, szTip);
    OFFSET(NOTIFYICONDATAW, dwState);
    OFFSET(NOTIFYICONDATAW, guidItem);
    OFFSET(NOTIFYICONDATAW, hBalloonIcon);
    SIZE(MENUITEMINFOW);
    OFFSET(MENUITEMINFOW, hSubMenu);
    OFFSET(MENUITEMINFOW, dwTypeData);
    OFFSET(MENUITEMINFOW, hbmpItem);
    SIZE(BITMAPINFOHEADER);
    OFFSET(BITMAPINFOHEADER, biCompression);
    return 0;
}
