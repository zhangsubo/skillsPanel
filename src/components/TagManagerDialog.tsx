import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Check, Loader2, Plus, Tag as TagIcon, Trash2 } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useTags } from '@/hooks/use-tags';
import { TagChip } from './TagChip';

/**
 * Full CRUD dialog for user-defined tags. Also used to:
 *  - create / delete existing tags (cascade warning on delete)
 *  - bulk-attach a tag to the currently-selected skills (if `selectedSkillIds` given)
 *
 * When `selectedSkillIds` has exactly one id, the dialog switches to "single
 * skill" mode: the per-tag apply button toggles between attach and detach,
 * using the live `tagsForSkill` result for that skill.
 */
interface TagManagerDialogProps {
  open: boolean;
  onClose: () => void;
  /** When non-empty, the dialog shows an apply button per tag. */
  selectedSkillIds?: string[];
  /** Optional pre-highlighted tag id. Cosmetic only — no focus/scroll. */
  focusTagId?: string;
}

/** Visually distinct palette for tag colors. */
const TAG_PALETTE = [
  '#ef4444', '#f97316', '#f59e0b', '#eab308', '#84cc16',
  '#22c55e', '#14b8a6', '#06b6d4', '#3b82f6', '#6366f1',
  '#8b5cf6', '#a855f7', '#d946ef', '#ec4899', '#f43f5e',
  '#64748b', '#78716c', '#0ea5e9', '#10b981', '#e11d48',
];

/** Minimum Euclidean distance in RGB space to consider two colors "different". */
const MIN_COLOR_DISTANCE = 80;

function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.replace('#', ''), 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

function colorDistance(a: string, b: string): number {
  const [r1, g1, b1] = hexToRgb(a);
  const [r2, g2, b2] = hexToRgb(b);
  return Math.sqrt((r1 - r2) ** 2 + (g1 - g2) ** 2 + (b1 - b2) ** 2);
}

/**
 * Pick a random color from TAG_PALETTE that is visually distinct from all
 * existing tag colors (Euclidean RGB distance >= MIN_COLOR_DISTANCE).
 * Falls back to any unused exact-match color, then any palette color.
 */
function pickRandomColor(existingColors: string[]): string {
  // Tier 1: visually distant from all existing colors
  const distant = TAG_PALETTE.filter(
    (c) => existingColors.every((e) => colorDistance(c, e) >= MIN_COLOR_DISTANCE),
  );
  if (distant.length > 0) {
    return distant[Math.floor(Math.random() * distant.length)];
  }
  // Tier 2: at least not an exact duplicate
  const unique = TAG_PALETTE.filter((c) => !existingColors.includes(c));
  if (unique.length > 0) {
    return unique[Math.floor(Math.random() * unique.length)];
  }
  // Tier 3: all taken, pick any
  return TAG_PALETTE[Math.floor(Math.random() * TAG_PALETTE.length)];
}

export function TagManagerDialog({ open, onClose, selectedSkillIds = [], focusTagId }: TagManagerDialogProps) {
  const { t } = useTranslation();
  const { tags, loading, create, remove, bulkAttach, attach, detach, tagsForSkill } = useTags();

  const [newName, setNewName] = useState('');
  const [newColor, setNewColor] = useState(() => pickRandomColor([]));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Single-skill mode: live mirror of that skill's tag set so the apply
  // button can show attach vs. attached state without parent plumbing.
  const singleMode = selectedSkillIds.length === 1;
  const singleSkillId = singleMode ? selectedSkillIds[0] : null;
  const [singleAttached, setSingleAttached] = useState<Set<string>>(new Set());
  const [singleLoading, setSingleLoading] = useState(false);

  const safeErrorMessage = (e: unknown): string =>
    e instanceof Error ? e.message : String(e);

  // Reset transient form state when the dialog opens/closes.
  useEffect(() => {
    if (open) {
      setNewName('');
      setNewColor(pickRandomColor(tags.map((t) => t.color).filter((c): c is string => c !== null)));
      setError(null);
    }
  }, [open]);

  // In single-skill mode, fetch the current attach set on open + on skill change.
  // Deliberately NOT dependent on `tags` (the hook's list) — tag list mutations
  // inside this dialog must not trigger an extra refetch that races with the
  // user's next attach/detach click.
  useEffect(() => {
    if (!open || !singleSkillId) {
      return;
    }
    let cancelled = false;
    setSingleLoading(true);
    // Clear the previous skill's attach set immediately to avoid a UI flash
    // where stale "Attached ✓" badges linger for the new skill.
    setSingleAttached(new Set());
    tagsForSkill(singleSkillId)
      .then((list) => {
        if (cancelled) return;
        setSingleAttached(new Set(list.map((tag) => tag.id)));
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setError(safeErrorMessage(e));
      })
      .finally(() => {
        if (!cancelled) setSingleLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, singleSkillId, tagsForSkill]);

  const handleCreate = async () => {
    const name = newName.trim();
    if (!name) return;
    setBusy(true);
    setError(null);
    try {
      await create(name, newColor, null);
      setNewName('');
    } catch (e) {
      setError(safeErrorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async (id: string) => {
    if (!window.confirm(t('tag.confirmDelete'))) return;
    setBusy(true);
    setError(null);
    try {
      await remove(id);
    } catch (e) {
      setError(safeErrorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const handleApplyToSelected = async (tagId: string) => {
    if (selectedSkillIds.length === 0) return;
    setBusy(true);
    setError(null);
    try {
      if (singleMode && singleSkillId) {
        if (singleAttached.has(tagId)) {
          await detach(singleSkillId, tagId);
          setSingleAttached((prev) => {
            const next = new Set(prev);
            next.delete(tagId);
            return next;
          });
        } else {
          await attach(singleSkillId, tagId);
          setSingleAttached((prev) => new Set(prev).add(tagId));
        }
      } else {
        await bulkAttach(selectedSkillIds, tagId);
      }
    } catch (e) {
      setError(safeErrorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const applyLabel = singleMode
    ? t('tag.applyToCurrent')
    : t('tag.applyToN', { count: selectedSkillIds.length });

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <TagIcon className="h-4 w-4" />
            {t('tag.manage')}
          </DialogTitle>
          <DialogDescription>{t('tag.manageDescription')}</DialogDescription>
        </DialogHeader>

        {/* Create form */}
        <div className="flex items-center gap-2">
          <Input
            placeholder={t('tag.namePlaceholder')}
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key !== 'Enter') return;
              if (busy || !newName.trim()) return;
              e.preventDefault();
              void handleCreate();
            }}
            className="flex-1"
          />
          <input
            type="color"
            value={newColor}
            onChange={(e) => setNewColor(e.target.value)}
            className="h-9 w-9 cursor-pointer rounded border border-input bg-transparent"
            aria-label={t('tag.colorLabel')}
            title={t('tag.colorLabel')}
          />
          <Button onClick={handleCreate} disabled={busy || !newName.trim()} size="sm">
            {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Plus className="h-3.5 w-3.5" />}
            <span className="ml-1">{t('tag.create')}</span>
          </Button>
        </div>

        {error && <p className="text-xs text-destructive">{error}</p>}

        {/* Tag list */}
        <div className="max-h-72 space-y-1 overflow-y-auto rounded-md border p-2">
          {loading && tags.length === 0 ? (
            <div className="flex items-center justify-center py-4 text-xs text-muted-foreground">
              <Loader2 className="mr-1 h-3.5 w-3.5 animate-spin" />
              {t('common.loading')}
            </div>
          ) : tags.length === 0 ? (
            <p className="py-4 text-center text-xs text-muted-foreground">{t('tag.empty')}</p>
          ) : (
            tags.map((tag) => {
              const attached = singleMode && singleAttached.has(tag.id);
              const toggleLabel = attached ? t('tag.detach') : t('tag.attach');
              return (
                <div
                  key={tag.id}
                  className={`flex items-center justify-between gap-2 rounded px-2 py-1.5 hover:bg-muted/50 ${
                    tag.id === focusTagId ? 'bg-muted/50' : ''
                  }`}
                >
                  <TagChip tag={tag} selected={attached} />
                  <div className="flex items-center gap-1">
                    {selectedSkillIds.length > 0 &&
                      (singleMode ? (
                        // Toggle button — a11y-valid only when action truly toggles state.
                        <Button
                          variant={attached ? 'secondary' : 'ghost'}
                          size="sm"
                          className="h-7 text-xs"
                          onClick={() => handleApplyToSelected(tag.id)}
                          disabled={busy || singleLoading}
                          aria-pressed={attached}
                          aria-label={`${toggleLabel} ${tag.name}`}
                          data-testid={`tag-apply-${tag.id}`}
                        >
                          {attached ? <Check className="mr-1 h-3.5 w-3.5" /> : <Plus className="mr-1 h-3.5 w-3.5" />}
                          {toggleLabel}
                        </Button>
                      ) : (
                        // Bulk mode — one-way attach. Plain button, no aria-pressed.
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-7 text-xs"
                          onClick={() => handleApplyToSelected(tag.id)}
                          disabled={busy}
                          aria-label={t('tag.applyToN', { count: selectedSkillIds.length })}
                          data-testid={`tag-apply-${tag.id}`}
                        >
                          <Plus className="mr-1 h-3.5 w-3.5" />
                          {applyLabel}
                        </Button>
                      ))}
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 text-muted-foreground hover:text-destructive"
                      onClick={() => handleDelete(tag.id)}
                      disabled={busy}
                      aria-label={t('tag.deleteAriaLabel', { name: tag.name })}
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </div>
              );
            })
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t('common.close')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
