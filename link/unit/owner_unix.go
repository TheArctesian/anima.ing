//go:build unix

package unit

import (
	"os"
	"path/filepath"
	"syscall"
)

// CheckProgram refuses a program that another user could replace: the unit would run it as this user, with the link's key.
func CheckProgram(program string, uid int) error {
	for _, path := range []string{program, filepath.Dir(program)} {
		entry, err := os.Lstat(path)
		if err != nil {
			return refused("%s cannot be read", path)
		}
		stat, ok := entry.Sys().(*syscall.Stat_t)
		if !ok || (int(stat.Uid) != uid && stat.Uid != 0) {
			return refused("%s does not belong to this user or to root", path)
		}
		if entry.Mode().Perm()&0o022 != 0 {
			return refused("%s can be changed by other users: remove the write permission of its group and of others", path)
		}
	}
	entry, err := os.Lstat(program)
	if err != nil || !entry.Mode().IsRegular() || entry.Mode().Perm()&0o100 == 0 {
		return refused("%s is not a program file", program)
	}
	return nil
}
