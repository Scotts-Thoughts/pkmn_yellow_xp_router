#!/usr/bin/env python3
"""
Script to update trainer money values in trainers.json based on:
- Trainer class (to get base_money_yield from trainer_prize_lookup.json)
- Last pokemon's level
- Formula: money = last_mon_level * base_money_yield
"""

import json
import os
from pathlib import Path

# File paths
SCRIPT_DIR = Path(__file__).parent
TRAINERS_FILE = SCRIPT_DIR / "raw_pkmn_data" / "gen_four" / "heartgold_soulsilver" / "trainers.json"
PRIZE_LOOKUP_FILE = SCRIPT_DIR / "raw_pkmn_data" / "gen_four" / "trainer_prize_lookup.json"


def update_trainer_money():
    """Update money values for all trainers in trainers.json"""
    
    # Load trainer prize lookup
    print(f"Loading prize lookup from: {PRIZE_LOOKUP_FILE}")
    with open(PRIZE_LOOKUP_FILE, 'r', encoding='utf-8') as f:
        prize_lookup = json.load(f)
    print(f"Loaded {len(prize_lookup)} trainer class entries\n")
    
    # Load trainers data
    print(f"Loading trainers from: {TRAINERS_FILE}")
    with open(TRAINERS_FILE, 'r', encoding='utf-8') as f:
        trainers_data = json.load(f)
    
    trainers = trainers_data.get('trainers', [])
    print(f"Loaded {len(trainers)} trainers\n")
    
    # Statistics
    updated_count = 0
    skipped_count = 0
    error_count = 0
    errors = []
    
    # Process each trainer
    for idx, trainer in enumerate(trainers, 1):
        trainer_class = trainer.get('trainer_class')
        pokemon_list = trainer.get('pokemon', [])
        
        # Skip if no trainer class
        if not trainer_class:
            skipped_count += 1
            errors.append(f"Trainer {idx}: Missing trainer_class")
            continue
        
        # Skip if no pokemon
        if not pokemon_list:
            skipped_count += 1
            errors.append(f"Trainer {idx} ({trainer_class}): No pokemon in list")
            continue
        
        # Get base money yield from lookup
        base_money_yield = prize_lookup.get(trainer_class)
        if base_money_yield is None:
            skipped_count += 1
            errors.append(f"Trainer {idx} ({trainer_class}): Class not found in prize lookup")
            continue
        
        # Get last pokemon's level
        last_pokemon = pokemon_list[-1]
        last_mon_level = last_pokemon.get('level')
        
        if last_mon_level is None:
            skipped_count += 1
            errors.append(f"Trainer {idx} ({trainer_class}): Last pokemon missing level")
            continue
        
        # Calculate new money value
        old_money = trainer.get('money', 0)
        new_money = last_mon_level * base_money_yield
        
        # Update money value
        trainer['money'] = new_money
        updated_count += 1
        
        # Log first few updates as examples
        if updated_count <= 5:
            print(f"Trainer {idx}: {trainer.get('trainer_name', 'Unknown')} ({trainer_class})")
            print(f"  Last pokemon level: {last_mon_level}")
            print(f"  Base money yield: {base_money_yield}")
            print(f"  Old money: {old_money} → New money: {new_money}")
            print()
    
    # Print summary
    print("=" * 80)
    print("UPDATE SUMMARY")
    print("=" * 80)
    print(f"Total trainers processed: {len(trainers)}")
    print(f"Successfully updated: {updated_count}")
    print(f"Skipped (errors): {skipped_count}")
    print(f"Errors: {error_count}")
    print()
    
    if errors:
        print("ERRORS ENCOUNTERED:")
        print("-" * 80)
        for error in errors[:20]:  # Show first 20 errors
            print(f"  {error}")
        if len(errors) > 20:
            print(f"  ... and {len(errors) - 20} more errors")
        print()
    
    # Save updated trainers.json
    if updated_count > 0:
        print(f"Saving updated trainers to: {TRAINERS_FILE}")
        with open(TRAINERS_FILE, 'w', encoding='utf-8') as f:
            json.dump(trainers_data, f, indent=4, ensure_ascii=False)
        print("✓ File saved successfully!")
    else:
        print("No trainers were updated. File not saved.")


if __name__ == "__main__":
    try:
        update_trainer_money()
    except FileNotFoundError as e:
        print(f"ERROR: File not found - {e}")
        print(f"Make sure you're running this script from the project root directory.")
    except json.JSONDecodeError as e:
        print(f"ERROR: Invalid JSON - {e}")
    except Exception as e:
        print(f"ERROR: {type(e).__name__} - {e}")
        import traceback
        traceback.print_exc()

