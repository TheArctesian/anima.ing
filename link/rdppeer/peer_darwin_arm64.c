//go:build darwin && arm64 && cgo

#include "peer.h"

#include <Security/Security.h>
#include <bsm/audit.h>
#include <sys/socket.h>
#include <sys/un.h>

int anima_rdp_host_peer(int socket) {
    audit_token_t audit;
    socklen_t length = sizeof audit;
    if (getsockopt(socket, SOL_LOCAL, LOCAL_PEERTOKEN, &audit, &length) != 0 || length != sizeof audit)
        return 0;

    CFDataRef token = CFDataCreate(NULL, (const UInt8 *)&audit, sizeof audit);
    if (!token) return 0;
    const void *keys[] = {kSecGuestAttributeAudit};
    const void *values[] = {token};
    CFDictionaryRef attributes = CFDictionaryCreate(NULL, keys, values, 1,
        &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    CFRelease(token);
    if (!attributes) return 0;

    SecCodeRef code = NULL;
    OSStatus status = SecCodeCopyGuestWithAttributes(NULL, attributes, kSecCSDefaultFlags, &code);
    CFRelease(attributes);
    if (status != errSecSuccess) return 0;

    // This is the Developer ID and application identifier of the installed desktop bundle.
    // The kernel audit token identifies the process on this socket without a path or PID lookup.
    CFStringRef expression = CFSTR("anchor apple generic and identifier \"ing.anima.desktop\""
        " and certificate 1[field.1.2.840.113635.100.6.2.6] exists"
        " and certificate leaf[field.1.2.840.113635.100.6.1.13] exists"
        " and certificate leaf[subject.OU] = \"7GDLG48PV6\"");
    SecRequirementRef requirement = NULL;
    status = SecRequirementCreateWithString(expression, kSecCSDefaultFlags, &requirement);
    if (status == errSecSuccess) {
        status = SecCodeCheckValidity(code, kSecCSDefaultFlags, requirement);
        CFRelease(requirement);
    }
    CFRelease(code);
    return status == errSecSuccess;
}
