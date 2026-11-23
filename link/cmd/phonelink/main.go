// Command phonelink is the device link as a C library, because an iPhone app cannot start a program (mobile plan M3-1, MOB-031).
// Each JSON answer goes back to AnimaLinkFree.
package main

/*
#include <stdlib.h>
*/
import "C"

import (
	"encoding/json"
	"unsafe"
)

func main() {}

// AnimaLinkJoin uses an invite and keeps the device's identity in the directory state; the invite is not logged (MOB-035, LOG-007).
//
//export AnimaLinkJoin
func AnimaLinkJoin(state, invite *C.char) *C.char {
	value, err := join(C.GoString(state), C.GoString(invite))
	return answer(value, err)
}

// AnimaLinkStart starts the link of the device in the directory state on the loopback address listen.
//
//export AnimaLinkStart
func AnimaLinkStart(state, listen *C.char) *C.char {
	value, err := start(C.GoString(state), C.GoString(listen))
	return answer(value, err)
}

// AnimaLinkStatus says whether the link runs, its local address and its path to the hub.
//
//export AnimaLinkStatus
func AnimaLinkStatus() *C.char { return answer(status(), nil) }

// AnimaLinkStop stops the link; it does nothing when the link does not run.
//
//export AnimaLinkStop
func AnimaLinkStop() { stop() }

// AnimaLinkFree frees an answer of this library.
//
//export AnimaLinkFree
func AnimaLinkFree(text *C.char) { C.free(unsafe.Pointer(text)) }

func answer(value any, err error) *C.char {
	if err != nil {
		value = failure{Error: code(err)}
	}
	text, marshalErr := json.Marshal(value)
	if marshalErr != nil {
		text = []byte(`{"error":"failed"}`)
	}
	return C.CString(string(text))
}
