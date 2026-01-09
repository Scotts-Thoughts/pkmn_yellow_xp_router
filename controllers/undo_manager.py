import logging
from typing import List, Optional
import copy

logger = logging.getLogger(__name__)


class UndoManager:
    """Manages undo/redo functionality for event list changes."""
    
    def __init__(self, max_steps: int = 15):
        self._max_steps = max_steps
        self._undo_stack: List[dict] = []
        self._current_state: Optional[dict] = None
    
    def save_state(self, router, is_post_operation=False):
        """Save the current event list state for undo.
        
        Args:
            router: The Router instance to serialize
            is_post_operation: If True, this is being called after an operation completes.
                             If False, this is being called before an operation (save to stack).
        """
        try:
            # Serialize the root folder (which contains all events)
            serialized_events = router.root_folder.serialize()
            
            # Serialize level up move defs - keys are tuples, need to convert to string for JSON compatibility
            level_up_move_defs_serialized = {}
            for key, move_def in router.level_up_move_defs.items():
                # Convert tuple key to string representation for storage
                key_str = str(key) if isinstance(key, tuple) else key
                level_up_move_defs_serialized[key_str] = move_def.serialize()
            
            # Create a state snapshot
            new_state = {
                'events': serialized_events,
                'defeated_trainers': list(router.defeated_trainers),
                'level_up_move_defs': level_up_move_defs_serialized
            }
            
            if is_post_operation:
                # After operation: just update current state (don't add to stack)
                # The pre-operation state was already added to the stack
                self._current_state = new_state
            else:
                # Before operation: save current state to stack, then update current state
                # This ensures we can undo to the state before this operation
                if self._current_state is not None:
                    self._undo_stack.append(self._current_state)
                    # Limit stack size
                    if len(self._undo_stack) > self._max_steps:
                        self._undo_stack.pop(0)
                
                # Update current state to the new state (which is the state before the operation)
                self._current_state = new_state
            
        except Exception as e:
            logger.error(f"Failed to save undo state: {e}")
            logger.exception(e)
    
    def can_undo(self) -> bool:
        """Check if undo is available."""
        return len(self._undo_stack) > 0
    
    def get_undo_state(self) -> Optional[dict]:
        """Get the previous state for undo, or None if not available."""
        if not self.can_undo():
            return None
        
        # Pop the last state from the stack
        previous_state = self._undo_stack.pop()
        
        # The current state becomes the new current state
        # (we'll restore it on undo)
        return previous_state
    
    def clear(self):
        """Clear all undo history."""
        self._undo_stack.clear()
        self._current_state = None

