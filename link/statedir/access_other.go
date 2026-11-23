//go:build !darwin || ios

package statedir

// dropInheritedAccess does nothing here: Linux mode bits bound an access-control list, and an iOS app cannot start chmod (MOB-031).
func dropInheritedAccess(string) error { return nil }
