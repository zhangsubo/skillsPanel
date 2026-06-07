import type { Tag } from '@/types';
import { X } from 'lucide-react';

interface TagChipProps {
  tag: Tag;
  /** When true, shows a small × button to remove the tag (used inside the manage dialog). */
  onRemove?: (tagId: string) => void;
  /** When true, clicking the chip triggers onClick. */
  onClick?: (tag: Tag) => void;
  /** When true, the chip is rendered with a slightly stronger background to indicate "selected". */
  selected?: boolean;
  className?: string;
}

/**
 * Compact pill that renders a single tag. Color is read from `tag.color`
 * and falls back to a neutral style when the user hasn't picked one.
 */
export function TagChip({ tag, onRemove, onClick, selected, className }: TagChipProps) {
  const hasColor = !!tag.color;
  const style = hasColor
    ? { backgroundColor: tag.color!, color: 'white' }
    : undefined;

  const baseClass = hasColor
    ? ''
    : 'bg-muted text-muted-foreground';

  const interactiveClass = onClick ? 'cursor-pointer hover:opacity-80' : '';

  return (
    <span
      onClick={onClick ? () => onClick(tag) : undefined}
      className={`inline-flex h-5 items-center gap-1 rounded-full px-2 text-[10px] font-medium ${baseClass} ${interactiveClass} ${selected ? 'ring-1 ring-primary' : ''} ${className ?? ''}`}
      style={style}
    >
      {tag.name}
      {onRemove && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onRemove(tag.id);
          }}
          className="ml-0.5 inline-flex h-3 w-3 items-center justify-center rounded-full hover:bg-black/20"
          aria-label={`Remove ${tag.name}`}
        >
          <X className="h-2.5 w-2.5" />
        </button>
      )}
    </span>
  );
}
