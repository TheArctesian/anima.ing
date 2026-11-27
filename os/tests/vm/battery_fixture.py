"""Synthetic laptop sysfs only inside disposable VMs; production readers stay unchanged."""
import shlex
import time


def install(machine, *, installed=False):
    machine.command("mkdir -p /run/battery-fixture/BAT0; printf 'Battery\\n' > /run/battery-fixture/BAT0/type; printf 'System\\n' > /run/battery-fixture/BAT0/scope; printf '1\\n' > /run/battery-fixture/BAT0/present; printf '73\\n' > /run/battery-fixture/BAT0/capacity; printf 'Charging\\n' > /run/battery-fixture/BAT0/status; mount --bind /run/battery-fixture /sys/class/power_supply")
    if installed:
        predicate = 'any((.batteries // [])[]; .name == "BAT0" and .percent == 73)'
        read = shlex.join(['jq', '-e', predicate, '/run/anima-observation/console.json'])
        wait = shlex.join(['timeout', '20', 'sh', '-c', f'until {read} 2>/dev/null; do sleep 1; done'])
        machine.command('systemctl restart anima-os-control && ' + wait)
    else:
        time.sleep(6)
