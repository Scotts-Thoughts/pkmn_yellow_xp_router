import sys
import os
import shutil
import re
import logging
import json
import time

from utils.constants import const

logger = logging.getLogger(__name__)


def is_likely_cloud_placeholder(file_path: str) -> bool:
    """Check if a file might be a cloud storage placeholder (Dropbox, iCloud, etc.).
    
    Cloud storage services on macOS use placeholder files that appear in the
    filesystem but aren't fully downloaded. These files often have a reported
    size > 0 but return empty content when read.
    """
    try:
        # Check if file exists
        if not os.path.exists(file_path):
            return False
        
        # Get reported file size
        stat_result = os.stat(file_path)
        reported_size = stat_result.st_size
        
        # If the file reports size 0, it's just empty, not a placeholder
        if reported_size == 0:
            return False
        
        # Try to read the file and check if content is empty
        with open(file_path, 'r') as f:
            content = f.read()
        
        # If reported size > 0 but content is empty, likely a placeholder
        if reported_size > 0 and len(content) == 0:
            return True
        
        return False
    except Exception:
        return False


def read_json_file_safe(file_path: str, max_wait_seconds: float = 2.0) -> dict:
    """Read a JSON file, handling cloud storage placeholder files.
    
    On macOS, cloud storage services (Dropbox, iCloud, OneDrive) may show files
    as placeholders that need to be downloaded. This function attempts to detect
    this situation and wait briefly for the download to complete.
    
    Args:
        file_path: Path to the JSON file
        max_wait_seconds: Maximum time to wait for placeholder download
    
    Returns:
        The parsed JSON data
    
    Raises:
        FileNotFoundError: If the file doesn't exist
        ValueError: If the file is empty or a cloud placeholder that couldn't be read
        json.JSONDecodeError: If the file contains invalid JSON
    """
    if not os.path.exists(file_path):
        raise FileNotFoundError(f"File not found: {file_path}")
    
    # Check if this might be a cloud placeholder
    stat_result = os.stat(file_path)
    reported_size = stat_result.st_size
    
    # First attempt to read
    with open(file_path, 'r') as f:
        content = f.read()
    
    # If we got content, parse it
    if content:
        return json.loads(content)
    
    # If reported size > 0 but content is empty, this is likely a cloud placeholder
    if reported_size > 0:
        # Wait a bit and retry - accessing the file may trigger download
        wait_time = 0.0
        wait_interval = 0.25
        
        while wait_time < max_wait_seconds:
            time.sleep(wait_interval)
            wait_time += wait_interval
            
            with open(file_path, 'r') as f:
                content = f.read()
            
            if content:
                return json.loads(content)
        
        # Still empty after waiting - likely a cloud placeholder that hasn't synced
        raise ValueError(
            f"File appears to be a cloud storage placeholder that hasn't been downloaded: {file_path}\n"
            f"If using Dropbox/iCloud/OneDrive, please ensure the file is set to 'Available offline' or has finished syncing."
        )
    
    # File is genuinely empty
    raise ValueError(f"File is empty: {file_path}")


def sanitize_string(string:str):
    if not isinstance(string, str):
        return string
    return ''.join([x for x in string if x.isalnum()]).lower()


def get_path_safe_string(raw_string):
    value = re.sub('[^\w\s-]', '', raw_string).strip().lower()
    value = re.sub('[-\s]+', '-', value)
    return value


def get_safe_path_no_collision(base_folder, name, ext=""):
    name = get_path_safe_string(name)
    result = os.path.join(base_folder, name) + ext
    if os.path.exists(result):
        counter = 0
        while os.path.exists(result):
            counter += 1
            result = os.path.join(base_folder, f"{name}_{counter}") + ext
    
    return result


def get_existing_route_path(route_name) -> str:
    result = os.path.join(const.SAVED_ROUTES_DIR, f"{route_name}.json")
    if not os.path.exists(result):
        result = os.path.join(const.OUTDATED_ROUTES_DIR, f"{route_name}.json")
    
    return result


def get_existing_route_names(filter_text="", load_backups=False):
    loaded_routes = []
    filter_text = filter_text.lower()

    if os.path.exists(const.SAVED_ROUTES_DIR):
        for fragment in os.listdir(const.SAVED_ROUTES_DIR):
            name, ext = os.path.splitext(fragment)
            if filter_text not in name.lower():
                continue
            if ext != ".json":
                continue
            loaded_routes.append(name)
    
    if load_backups:
        if os.path.exists(const.OUTDATED_ROUTES_DIR):
            for fragment in os.listdir(const.OUTDATED_ROUTES_DIR):
                name, ext = os.path.splitext(fragment)
                if filter_text not in name.lower():
                    continue
                if ext != ".json":
                    continue
                loaded_routes.append(name)

    return sorted(loaded_routes, key=str.casefold)


def change_user_data_location(orig_dir, new_dir) -> bool:
    try:
        # If the orig dir is invalid for some reason, assume this is first time setup
        # just create the new dir, and return
        if not orig_dir or not os.path.exists(orig_dir):
            if not os.path.exists(new_dir):
                os.makedirs(new_dir)
            return True

        if not os.path.exists(new_dir):
            os.makedirs(new_dir)
        
        for orig_inner_dir, new_inner_dir in const.get_potential_user_data_dirs(new_dir):
            if os.path.exists(orig_inner_dir):
                shutil.copytree(orig_inner_dir, new_inner_dir)
        
        # do these separately so we only remove files after everything has been copied to the new location
        for orig_inner_dir, _ in const.get_potential_user_data_dirs(new_dir):
            if os.path.exists(orig_inner_dir):
                shutil.rmtree(orig_inner_dir)
        
        # Only nuke the previous dir if it's now empty
        if len(os.listdir(orig_dir)) == 0:
            shutil.rmtree(orig_dir)

        return True
    except Exception as e:
        logger.error(f"Failed to change data location to: {new_dir}")
        logger.exception(e)
        return False


def migrate_dir(orig_dir, new_dir) -> bool:
    try:
        shutil.move(orig_dir, new_dir)
        return True
    except Exception as e:
        logger.error(f"Failed to change migrate dir from: {orig_dir} to: {new_dir}")
        logger.exception(e)
        return False


def open_explorer(path) -> bool:
    try:
        if sys.platform == "linux" or sys.platform == "linux2":
            os.system(f'xdg-open "{path}"')
        elif sys.platform == "darwin":
            os.system(f'open "{path}"')
        elif sys.platform == "win32":
            os.startfile(path)
        else:
            return False
        
        return True
    except Exception as e:
        logger.error(f"Failed to open explorer to location: {path}")
        logger.exception(e)
        return False


def get_default_user_data_dir():
    result = os.path.expanduser("~")
    test = os.path.join(result, "Documents")
    if os.path.exists(test):
        result = test
    
    return os.path.join(result, const.APP_DATA_FOLDER_DEFAULT_NAME)


def backup_file_if_exists(orig_path):
    if os.path.exists(orig_path) and os.path.isfile(orig_path):
        new_backup_loc = get_safe_backup_path(orig_path)
        shutil.move(orig_path, new_backup_loc)


def get_safe_backup_path(orig_path):
    # first thing, convert the original path to the outdated folder, so that we don't clutter the main folder with backups
    _, orig_name = os.path.split(orig_path)
    orig_path = os.path.join(const.OUTDATED_ROUTES_DIR, orig_name)

    # TODO: kind of an awkward place for this... should it go somewhere else?
    if not os.path.exists(const.OUTDATED_ROUTES_DIR):
        os.makedirs(const.OUTDATED_ROUTES_DIR)

    base, ext = os.path.splitext(orig_path)
    counter = 1
    while True:
        result = f"{base}_{counter}{ext}"
        if not os.path.exists(result):
            return result
        counter += 1
