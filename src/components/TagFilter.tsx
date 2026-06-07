import { useTranslation } from 'react-i18next';
import type { Tag } from '@/types';

/** Sentinel value for the "untagged" filter option. */
export const UNTAGGED_FILTER = '__untagged__';

interface TagFilterProps {
  tags: Tag[];
  /** tag id currently selected; null = "all", UNTAGGED_FILTER = "no tags". */
  value: string | null;
  onChange: (tagId: string | null) => void;
  disabled?: boolean;
}

/**
 * Compact "All / Untagged / <tag> / <tag>" picker. Renders as a row of
 * toggleable buttons so it fits inline next to the Library search box.
 */
export function TagFilter({ tags, value, onChange, disabled }: TagFilterProps) {
  const { t } = useTranslation();

  if (tags.length === 0) return null;

  const btnBase = 'inline-flex h-7 items-center rounded-full px-3 text-xs font-medium transition-colors';
  const btnActive = 'bg-primary text-primary-foreground';
  const btnInactive = 'bg-muted text-muted-foreground hover:bg-muted/70';

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <button
        type="button"
        disabled={disabled}
        onClick={() => onChange(null)}
        className={`${btnBase} ${value === null ? btnActive : btnInactive}`}
      >
        {t('tag.filterAll')}
      </button>
      <button
        type="button"
        disabled={disabled}
        onClick={() => onChange(value === UNTAGGED_FILTER ? null : UNTAGGED_FILTER)}
        className={`${btnBase} ${value === UNTAGGED_FILTER ? btnActive : btnInactive}`}
      >
        {t('tag.filterUntagged')}
      </button>
      {tags.map((tag) => {
        const active = value === tag.id;
        const colorStyle = active && tag.color ? { backgroundColor: tag.color, color: 'white' } : undefined;
        return (
          <button
            key={tag.id}
            type="button"
            disabled={disabled}
            onClick={() => onChange(active ? null : tag.id)}
            className={`${btnBase} ${
              active
                ? tag.color ? '' : btnActive
                : tag.color ? 'text-foreground hover:opacity-80' : btnInactive
            }`}
            style={colorStyle}
          >
            {tag.name}
          </button>
        );
      })}
    </div>
  );
}
