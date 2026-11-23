"""Run a build on at most half of the CPUs available to this process."""
import ctypes
import os
import shutil
import subprocess
import sys


def limit_cpus():
    if sys.platform == "win32":
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.GetCurrentProcess.restype = ctypes.c_void_p
        mask_type = ctypes.c_size_t
        kernel.GetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.POINTER(mask_type), ctypes.POINTER(mask_type)]
        kernel.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, mask_type]
        process = kernel.GetCurrentProcess()
        available, system = mask_type(), mask_type()
        if not kernel.GetProcessAffinityMask(process, ctypes.byref(available), ctypes.byref(system)):
            raise ctypes.WinError(ctypes.get_last_error())
        cores = [1 << bit for bit in range(available.value.bit_length()) if available.value & (1 << bit)]
        count = max(1, len(cores) // 2)
        if not kernel.SetProcessAffinityMask(process, sum(cores[:count])):
            raise ctypes.WinError(ctypes.get_last_error())
    else:
        cores = sorted(os.sched_getaffinity(0))
        count = max(1, len(cores) // 2)
        os.sched_setaffinity(0, cores[:count])
    print(f"Build CPU affinity: {count}/{len(cores)} logical CPUs", flush=True)
    return count


if __name__ == "__main__":
    count = limit_cpus()
    os.environ["CARGO_BUILD_JOBS"] = str(count)
    if len(sys.argv) < 2 or not (program := shutil.which(sys.argv[1])):
        sys.exit("Provide a build command available on PATH.")
    args = sys.argv[2:]
    if sys.argv[1] == "gradle":
        args = [f"--max-workers={count}", *args]
        os.environ["JAVA_TOOL_OPTIONS"] = (os.environ.get("JAVA_TOOL_OPTIONS", "") + f" -XX:ActiveProcessorCount={count}").strip()
    sys.exit(subprocess.call([program, *args]))
