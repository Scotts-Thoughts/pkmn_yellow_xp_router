
import os
import sys
import subprocess
import zipfile
import platform
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


def normalize_path_for_pyinstaller(path):
    """Convert Windows paths to forward slashes for PyInstaller on macOS"""
    return path.replace('\\', '/')


if __name__ == "__main__":
    # This script must be run on macOS to create a macOS app bundle
    if platform.system() != 'Darwin':
        print("ERROR: This script must be run on macOS to create a macOS .app bundle.")
        print("PyInstaller creates platform-specific builds and cannot create macOS apps on Windows or Linux.")
        sys.exit(1)
    
    root_path = os.path.dirname(os.path.abspath(__file__))
    
    # Determine app and zip names based on development mode
    if const.DEVELOPMENT_MODE:
        timestamp = datetime.now().strftime("%Y%m%d%H%M")
        app_name = f"pkmn_xp_router_{timestamp}"
        zip_name = f"pkmn_xp_router_{timestamp}.zip"
    else:
        app_name = "pkmn_xp_router"
        zip_name = f"pkmn_xp_router_{const.APP_VERSION}.zip"
    
    cmd = [
        sys.executable,
        "-m",
        "PyInstaller",
        "main.pyw",
        "--noconfirm",
        "--onefile",
        "--windowed",
        "--hidden-import=PIL",
        "--hidden-import=appdirs",
        "--hidden-import=signalrcore",
        "--name", app_name,
        "--add-data", normalize_path_for_pyinstaller("assets/*.tcl") + ":assets",
        "--add-data", normalize_path_for_pyinstaller("assets/theme/*.tcl") + ":assets/theme",
        "--add-data", normalize_path_for_pyinstaller("assets/theme/dark/*") + ":assets/theme/dark",
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
        # Normalize paths to use forward slashes for PyInstaller
        normalized_folder = normalize_path_for_pyinstaller(cur_json_folder)
        cmd.extend([
            "--add-data",
            f"{normalized_folder}/*.json:{normalized_folder}"
        ])

    print(f"cmd: {' '.join(cmd)}")
    subprocess.call(cmd)

    # Create zip file containing the macOS app bundle
    dist_dir = os.path.join(root_path, "dist")
    app_path = os.path.join(dist_dir, f"{app_name}.app")
    zip_path = os.path.join(dist_dir, zip_name)
    
    if os.path.exists(app_path):
        print(f"Creating zip file: {zip_path}")
        with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED) as zipf:
            # Add the entire app bundle to the zip
            for root, dirs, files in os.walk(app_path):
                for file in files:
                    file_path = os.path.join(root, file)
                    arcname = os.path.relpath(file_path, dist_dir)
                    zipf.write(file_path, arcname)
        print(f"Zip file created successfully: {zip_path}")
    else:
        print(f"ERROR: App bundle not found at {app_path}.")
        print("The build may have failed. Check the PyInstaller output above for errors.")
        sys.exit(1)

