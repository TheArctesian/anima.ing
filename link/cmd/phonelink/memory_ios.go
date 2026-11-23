package main

/*
#include <mach/mach.h>
#include <stddef.h>
#include <stdint.h>
#include <sys/sysctl.h>
#include <unistd.h>

static inline int memory_sample(uint64_t *total, uint64_t *used) {
    size_t length = sizeof(*total);
    if (sysctlbyname("hw.memsize", total, &length, NULL, 0) != 0 ||
        length != sizeof(*total)) {
        return 0;
    }

    mach_port_t host = mach_host_self();
    vm_statistics64_data_t vm;
    mach_msg_type_number_t count = HOST_VM_INFO64_COUNT;
    kern_return_t result = host_statistics64(host, HOST_VM_INFO64, (host_info64_t)&vm, &count);
    mach_port_deallocate(mach_task_self(), host);
    const size_t needed = offsetof(vm_statistics64_data_t, internal_page_count) + sizeof(vm.internal_page_count);
    if (result != KERN_SUCCESS || count * sizeof(integer_t) < needed) {
        return 0;
    }

    uint64_t app = vm.internal_page_count > vm.purgeable_count
        ? vm.internal_page_count - vm.purgeable_count
        : 0;
    *used = (app + vm.wire_count + vm.compressor_page_count) * (uint64_t)getpagesize();
    return 1;
}
*/
import "C"

// AnimaLinkMemory writes device-wide memory in bytes to the caller.
//
//export AnimaLinkMemory
func AnimaLinkMemory(total, used *C.uint64_t) C.int {
	return C.memory_sample(total, used)
}
