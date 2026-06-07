import { useTranslation } from 'react-i18next';
import type { Tag } from '@/types';

interface TagFilterProps {
  tags: Tag[];
  /** tag id currently selected; null = "all". */
  value: string | null;
  onChange: (tagId: string | null) => void;
  disabled?: boolean;
}

/**
 * Compact "All / <tag> / <tag>" picker. Renders as a row of toggleable
 * buttons so it fits inline next to the Library search box.
 */
export function TagFilter({ tags, value, onChange, disabled }: TagFilterProps) {
  const { t } = useTranslation();

  if (tags.length === 0) return null;

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <button
        type="button"
        disabled={disabled}
        onClick={() => onChange(null)}
        className={`inline-flex h-7 items-center rounded-full px-3 text-xs font-medium transition-colors ${
          value === null
            ? 'bg-primary text-primary-foreground'
            : 'bg-muted text-muted-foreground hover:bg-muted/70'
        }`}
      >
        {t('tag.filterAll')}
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
            className={`inline-flex h-7 items-center rounded-full px-3 text-xs font-medium transition-colors ${
              active
                ? tag.color
                  ? ''
                  : 'bg-primary text-primary-foreground'
                : tag.color
                  ? 'text-foreground hover:opacity-80'
                  : 'bg-muted text-muted-foreground hover:bg-muted/70'
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
