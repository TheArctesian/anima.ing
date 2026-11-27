"""Installed-console acceptance through rendered controls, using disposable VM credentials."""
import time
import subprocess
from gui_test import qmp, screenshot
from gui_actions import locate


def key(output, code):
    qmp(output, 'send-key', {'keys': [{'type': 'qcode', 'data': code}], 'hold-time': 20})
    time.sleep(.04)


def click(machine, x, y):
    machine.command(f'DISPLAY=:0 XAUTHORITY=/home/admin/.Xauthority xdotool mousemove {x} {y} click 1')
    time.sleep(.3)


def text(output, value):
    for char in value:
        key(output, char)


def wait_label(output, phrase, timeout=60):
    deadline = time.monotonic()+timeout
    while time.monotonic() < deadline:
        try:
            return locate(output, phrase)
        except ValueError:
            time.sleep(1)
    raise RuntimeError('Installed UI did not show: '+phrase)


def wait_blank(output, timeout=30):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        screenshot(output, 'console-screen-off')
        brightness = subprocess.check_output(
            ['magick', str(output/'console-screen-off.ppm'), '-format', '%[fx:mean]', 'info:'],
            text=True,
        )
        if float(brightness) < .001:
            return
        time.sleep(.5)
    raise AssertionError('Screen-off must visibly blank the display')


def installed_console(machine, output, password, with_data_volume=False):
    wait_label(output, 'Local session locked')
    screenshot(output, 'console-locked')
    text(output, 'wrong'); key(output, 'ret')
    wait_label(output, 'Authentication failed')
    text(output, password); key(output, 'ret')
    wait_label(output, 'Local resources')
    screenshot(output, 'console-overview')
    x, y, w, h = locate(output, 'Activity'); click(machine, x+w//2, y+h//2)
    wait_label(output, 'Service events since boot')
    wait_label(output, 'Started')
    screenshot(output, 'console-activity')
    x, y, w, h = locate(output, 'Network'); click(machine, x+w//2, y+h//2)
    wait_label(output, 'No Wi-Fi adapter detected')
    screenshot(output, 'console-network')
    filesystem_bytes = None
    if with_data_volume:
        from storage_checks import check_storage_view
        filesystem_bytes = check_storage_view(machine, output, password)
    x, y, w, h = locate(output, 'Settings'); click(machine, x+w//2, y+h//2)
    wait_label(output, 'Display & accessibility')
    x, y, w, h = locate(output, 'Display & accessibility'); click(machine, x+w//2, y+h//2)
    wait_label(output, 'Orientation')
    screenshot(output, 'console-display')
    machine.command('DISPLAY=:0 XAUTHORITY=/home/admin/.Xauthority xdotool mousemove 800 500 click --repeat 8 --delay 30 5')
    x, y, w, h = wait_label(output, 'Turn screen off'); click(machine, x+w//2, y+h//2)
    wait_blank(output)
    machine.command('systemctl is-active NetworkManager; systemctl is-active display-manager')
    key(output, 'shift')
    wait_label(output, 'Local session locked')
    machine.command('systemctl is-active NetworkManager; systemctl is-active display-manager')
    screenshot(output, 'console-relocked')
    return filesystem_bytes


def installed_wifi(machine, password):
    machine.send('sudo -i'); machine.expect(r'password for admin:'); machine.send(password)
    machine.shell('root')
    try:
        machine.command("findmnt -n -o OPTIONS / | tr ',' '\\n' | grep -qx ro; systemctl restart wpa_supplicant || { journalctl -u wpa_supplicant --no-pager -n 25; exit 1; }; busctl get-property fi.w1.wpa_supplicant1 /fi/w1/wpa_supplicant1 fi.w1.wpa_supplicant1 Interfaces", timeout=60)
    finally:
        machine.send('exit'); machine.shell('admin')


def installed_orientation(machine):
    machine.command("test -r /persist/display.json; timeout 90 sh -c 'until DISPLAY=:0 XAUTHORITY=/home/admin/.Xauthority anima-display --list >/dev/null 2>&1; do sleep 1; done'; test \"$(DISPLAY=:0 XAUTHORITY=/home/admin/.Xauthority anima-display --list)\" = \"$(jq -r 'to_entries[] | [.key,.value] | @tsv' /persist/display.json)\"", timeout=100)


def installed_terminal_prompts(machine):
    machine.command("plain=$(starship prompt --status 0); "
                    "printf '%s' \"$plain\" | grep -F 'exit 0'; "
                    "! printf '%s' \"$plain\" | LC_ALL=C grep '[^[:print:][:space:]]'")
    machine.command("if (exit 37); then exit 1; else "
                    "STARSHIP_CONFIG=/etc/anima/starship-powerline.toml starship prompt --status \"$?\"; "
                    "fi | grep -F 'exit 37'")
