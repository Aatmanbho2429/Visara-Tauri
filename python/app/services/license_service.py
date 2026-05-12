import platform
import subprocess
import hashlib


def _run_command(cmd: str) -> str:
    try:
        result = subprocess.check_output(
            cmd, shell=True, stderr=subprocess.DEVNULL
        ).decode(errors="ignore").strip()
        return result
    except Exception:
        return ""


def _get_windows_ids():
    uuid  = _run_command("wmic csproduct get uuid").splitlines()
    cpu   = _run_command("wmic cpu get processorid").splitlines()
    disk  = _run_command("wmic diskdrive get serialnumber").splitlines()

    uuid_val  = uuid[1].strip()  if len(uuid)  > 1 else "UNKNOWN_UUID"
    cpu_val   = cpu[1].strip()   if len(cpu)   > 1 else "UNKNOWN_CPU"
    disk_val  = disk[1].strip()  if len(disk)  > 1 else "UNKNOWN_DISK"

    return [uuid_val, cpu_val, disk_val]


def _get_macos_ids():
    hw_uuid = _run_command(
        "ioreg -rd1 -c IOPlatformExpertDevice | awk '/IOPlatformUUID/ { print $3 }'"
    ).replace('"', "")
    serial = _run_command(
        "system_profiler SPHardwareDataType | awk '/Serial Number/ { print $4 }'"
    )
    return [
        hw_uuid or "UNKNOWN_HW_UUID",
        serial  or "UNKNOWN_SERIAL",
    ]


def get_device_id() -> str:
    os_name = platform.system()

    if os_name == "Windows":
        parts = _get_windows_ids()
    elif os_name == "Darwin":
        parts = _get_macos_ids()
    else:
        parts = ["UNSUPPORTED_OS"]

    raw = "|".join(parts).encode("utf-8")
    return hashlib.sha256(raw).hexdigest()
