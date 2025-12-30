import json
from collections import Counter

# Load the trainer prize lookup
with open('raw_pkmn_data/gen_four/trainer_prize_lookup.json', 'r') as f:
    prize_lookup = json.load(f)

# Load the trainers data
with open('raw_pkmn_data/gen_four/platinum/trainers.json', 'r') as f:
    trainers_data = json.load(f)

# Extract all unique trainer classes from trainers.json
trainer_classes_in_data = set()
trainer_class_counts = Counter()

for trainer in trainers_data['trainers']:
    trainer_class = trainer.get('trainer_class')
    if trainer_class:
        trainer_classes_in_data.add(trainer_class)
        trainer_class_counts[trainer_class] += 1

# Get all keys from prize lookup (normalized for comparison)
prize_lookup_keys = set(prize_lookup.keys())

# Find exact matches
exact_matches = trainer_classes_in_data & prize_lookup_keys

# Find discrepancies - classes in trainers.json that don't match any key in lookup
missing_in_lookup = trainer_classes_in_data - prize_lookup_keys

# Find classes in lookup that aren't used in trainers.json
unused_in_lookup = prize_lookup_keys - trainer_classes_in_data

# Check for case-insensitive matches (potential case issues)
case_insensitive_matches = {}
for trainer_class in missing_in_lookup:
    for lookup_key in prize_lookup_keys:
        if trainer_class.lower() == lookup_key.lower():
            case_insensitive_matches[trainer_class] = lookup_key
            break

# Print results
print("=" * 80)
print("TRAINER CLASS SPELLING DISCREPANCY REPORT")
print("=" * 80)
print(f"\nTotal unique trainer classes in trainers.json: {len(trainer_classes_in_data)}")
print(f"Total keys in trainer_prize_lookup.json: {len(prize_lookup_keys)}")
print(f"Exact matches: {len(exact_matches)}")
print(f"\n{'=' * 80}\n")

if missing_in_lookup:
    print("❌ DISCREPANCIES FOUND - Trainer classes in trainers.json NOT in lookup:")
    print("-" * 80)
    for trainer_class in sorted(missing_in_lookup):
        count = trainer_class_counts[trainer_class]
        print(f"  '{trainer_class}' (appears {count} times)")
        if trainer_class in case_insensitive_matches:
            print(f"    ⚠️  Case-insensitive match found: '{case_insensitive_matches[trainer_class]}'")
    print()
else:
    print("✅ All trainer classes in trainers.json have exact matches in lookup!\n")

if case_insensitive_matches:
    print("⚠️  CASE-INSENSITIVE MATCHES (potential case spelling issues):")
    print("-" * 80)
    for trainer_class, lookup_key in sorted(case_insensitive_matches.items()):
        print(f"  '{trainer_class}' → '{lookup_key}'")
    print()

if unused_in_lookup:
    print("ℹ️  Classes in lookup.json that aren't used in trainers.json:")
    print("-" * 80)
    for lookup_key in sorted(unused_in_lookup):
        print(f"  '{lookup_key}'")
    print()

# Summary of all classes with counts
print("=" * 80)
print("ALL TRAINER CLASSES IN trainers.json (with counts):")
print("=" * 80)
for trainer_class, count in sorted(trainer_class_counts.items()):
    status = "✅" if trainer_class in exact_matches else "❌"
    print(f"  {status} '{trainer_class}': {count} occurrences")