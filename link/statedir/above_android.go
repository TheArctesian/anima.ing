//go:build android

package statedir

// foldersAbove does nothing on Android: the system's folders above the app's data folder are group-writable by design (MOB-031).
func foldersAbove(string) error { return nil }
