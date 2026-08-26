// Which comment (if any) should grab focus the moment its card mounts —
// set right after "Add Comment" creates one, so the user can start typing
// immediately instead of having to click into the new note first. Module-
// level singleton, same pattern as dropdownRegistry.ts/useTheme.ts.
import { ref } from 'vue';

export const pendingFocusCommentId = ref<string | null>(null);

export function focusCommentOnMount(id: string): void {
  pendingFocusCommentId.value = id;
}

/** Called by CommentCard.vue on mount — consumes the pending focus request
 * if it's this card's, so it only ever fires once. */
export function consumePendingFocus(id: string): boolean {
  if (pendingFocusCommentId.value !== id) return false;
  pendingFocusCommentId.value = null;
  return true;
}
