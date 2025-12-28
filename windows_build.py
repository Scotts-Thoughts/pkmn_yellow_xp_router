
import os
import sys
import subprocess
import zipfile
from datetime import datetime
from utils.constants import const

def get_all_json_folders(root_path, ignore_dirs):
    result = []

    for test_dir, _, test_files in os.walk(root_path):
        do_ignore = False
        for test_ignore in ignore_dirs:
            if test_dir.startswith(test_ignore):
                do_ignore = True
                break
        
        if do_ignore:
            continue

        for cur_test in test_files:
            if cur_test.endswith(".json"):
                result.append(test_dir[len(root_path) + 1:])
                break
    
    return result


if __name__ == "__main__":
    root_path = os.path.dirname(os.path.abspath(__file__))
    
    # Determine exe and zip names based on development mode
    if const.DEVELOPMENT_MODE:
        timestamp = datetime.now().strftime("%Y%m%d%H%M")
        exe_name = f"pkmn_xp_router_{timestamp}"
        zip_name = f"pkmn_xp_router_{timestamp}.zip"
    else:
        exe_name = "pkmn_xp_router"
        zip_name = f"pkmn_xp_router_{const.APP_VERSION}.zip"
    
    cmd = [
        sys.executable,
        "-m",
        "PyInstaller",
        "main.pyw",
        "--noconfirm",
        "--onefile",
        "--hidden-import=PIL",
        "--hidden-import=appdirs",
        "--hidden-import=signalrcore",
        "--name", exe_name,
        "--add-data", "assets\*.tcl;assets",
        "--add-data", "assets\\theme\\*.tcl;assets\\theme",
        "--add-data", "assets\\theme\\dark\\*;assets\\theme\\dark",
    ]

    root_path = os.path.dirname(os.path.abspath(__file__))
    ignore_dirs = [
        os.path.join(root_path, x) for x in [
            ".git",
            "__pycache__",
            "build",
            "dist",
            "outdated_routes",
            "outputs",
            "route_one_output",
            "saved_routes"
        ]
    ]

    for cur_json_folder in get_all_json_folders(root_path, ignore_dirs):
        cmd.extend([
            "--add-data",
            f"{os.path.join(cur_json_folder, '*.json')};{cur_json_folder}"
        ])

    print(f"cmd: {' '.join(cmd)}")
    subprocess.call(cmd)

    # Create zip file containing the exe
    dist_dir = os.path.join(root_path, "dist")
    exe_path = os.path.join(dist_dir, f"{exe_name}.exe")
    zip_path = os.path.join(dist_dir, zip_name)
    
    if os.path.exists(exe_path):
        print(f"Creating zip file: {zip_path}")
        with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED) as zipf:
            zipf.write(exe_path, f"{exe_name}.exe")
        print(f"Zip file created successfully: {zip_path}")
    else:
        print(f"Warning: Exe file not found at {exe_path}. Skipping zip creation.")