package unit

import (
	"context"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"time"
)

const stepDeadline = 30 * time.Second

// Runner runs one step and reports success; the error means the step could not run.
type Runner func(ctx context.Context, step Step) (bool, error)

// Run is the Runner of the real system.
func Run(ctx context.Context, step Step) (bool, error) {
	ctx, cancel := context.WithTimeout(ctx, stepDeadline)
	defer cancel()
	command := exec.CommandContext(ctx, step.Program, step.Args...)
	env, directory := commandEnvironment()
	command.Env = append(env, step.Env...)
	command.Dir = directory
	err := command.Run()
	var exit *exec.ExitError
	if errors.As(err, &exit) && ctx.Err() == nil {
		return false, nil
	}
	return err == nil, err
}

func runAll(ctx context.Context, run Runner, steps []Step) error {
	for _, step := range steps {
		ok, err := run(ctx, step)
		if !ok && err == nil && step.RetryAfter > 0 {
			select {
			case <-ctx.Done():
				return ctx.Err()
			case <-time.After(step.RetryAfter):
			}
			ok, err = run(ctx, step)
		}
		if step.MayFail || (ok && err == nil) {
			continue
		}
		if err != nil {
			return fmt.Errorf("unit: %s %v: %w", step.Program, step.Args, err)
		}
		return fmt.Errorf("unit: %s %v failed", step.Program, step.Args)
	}
	return nil
}

// Install writes the unit's file and starts it, restarting a running unit so it uses the new file.
func Install(ctx context.Context, plan Plan, run Runner) error {
	if err := prepareDirectory(filepath.Dir(plan.File)); err != nil {
		return err
	}
	// The rename keeps a reader from finding half a file.
	next := plan.File + ".next"
	if err := os.WriteFile(next, unitBytes(plan.Content), 0o644); err != nil {
		return err
	}
	if err := os.Rename(next, plan.File); err != nil {
		_ = os.Remove(next)
		return err
	}
	return runAll(ctx, run, plan.Start)
}

// Remove stops the unit and deletes its file; an absent unit is not an error.
func Remove(ctx context.Context, plan Plan, run Runner) error {
	if err := runAll(ctx, run, plan.Stop); err != nil {
		return err
	}
	if err := os.Remove(plan.File); err != nil && !errors.Is(err, fs.ErrNotExist) {
		return err
	}
	return runAll(ctx, run, plan.After)
}

// Installed reports whether the file of the unit is there.
func Installed(plan Plan) bool {
	_, err := os.Lstat(plan.File)
	return err == nil
}
