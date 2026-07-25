import { useState } from "react";
import { NavLink, useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { LayoutDashboard, Package, Download, Settings as SettingsIcon, FolderKanban, Plus } from "lucide-react";
import { useProjects } from "@/hooks/use-projects";

const navKeys = [
  { to: "/", labelKey: "nav.dashboard", icon: LayoutDashboard },
  { to: "/library", labelKey: "nav.library", icon: Package },
  { to: "/scanner", labelKey: "nav.scanner", icon: Download },
] as const;

export default function Sidebar() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { projects, selectedProjectId, selectProject, projectSkillCounts } = useProjects();
  const [showAddDialog, setShowAddDialog] = useState(false);

  return (
    <aside className="flex w-[260px] shrink-0 flex-col border-r border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-900">
      <div className="flex h-14 items-center border-b border-gray-200 px-5 dark:border-gray-800">
        <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          {t('app.title')}
        </h1>
      </div>

      <nav className="space-y-1 px-3 py-4">
        {navKeys.map(({ to, labelKey, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            end={to === "/"}
            className={({ isActive }) =>
              [
                "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                isActive
                  ? "bg-gray-100 text-gray-900 dark:bg-gray-800 dark:text-gray-100"
                  : "text-gray-600 hover:bg-gray-50 hover:text-gray-900 dark:text-gray-400 dark:hover:bg-gray-800/50 dark:hover:text-gray-100",
              ].join(" ")
            }
          >
            <Icon className="h-4 w-4" />
            {t(labelKey)}
          </NavLink>
        ))}
      </nav>

      {/* ── 项目工作区分组 ─────────────────────────────────────── */}
      <div className="border-t border-gray-200 dark:border-gray-800">
        <div className="flex items-center justify-between px-5 py-3">
          <span className="text-xs font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">
            {t('nav.projectWorkspace')}
          </span>
          <button
            onClick={() => setShowAddDialog(true)}
            className="rounded p-0.5 text-gray-400 transition-colors hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-800 dark:hover:text-gray-300"
            title={t('project.addProject')}
          >
            <Plus className="h-3.5 w-3.5" />
          </button>
        </div>

        <div className="max-h-[240px] overflow-y-auto px-3 pb-2">
          {projects.length === 0 ? (
            <p className="px-2 py-1 text-xs text-gray-400 dark:text-gray-500">
              {t('project.noProjects')}
            </p>
          ) : (
            projects.map((project, index) => {
              const isSelected = selectedProjectId === project.id;
              const skillCount = projectSkillCounts[project.id];
              return (
                <div key={project.id}>
                  <button
                    onClick={() => {
                      selectProject(project.id);
                      navigate(`/projects/${project.id}`);
                    }}
                    className={[
                      "flex w-full items-center gap-2.5 rounded-md px-2.5 py-2 text-left text-sm transition-colors",
                      isSelected
                        ? "bg-gray-100 text-gray-900 dark:bg-gray-800 dark:text-gray-100"
                        : "text-gray-600 hover:bg-gray-50 hover:text-gray-900 dark:text-gray-400 dark:hover:bg-gray-800/50 dark:hover:text-gray-100",
                    ].join(" ")}
                  >
                    <FolderKanban className="h-4 w-4 shrink-0" />
                    <span className="min-w-0 flex-1 break-words leading-snug">
                      {project.name}
                    </span>
                    <span className="shrink-0 rounded-full bg-gray-100 px-1.5 py-0.5 text-[10px] font-medium text-gray-500 dark:bg-gray-700 dark:text-gray-400">
                      {skillCount ?? '—'}
                    </span>
                  </button>
                  {index < projects.length - 1 && (
                    <div className="my-0.5 mx-2.5 border-t border-dashed border-gray-200 dark:border-gray-700" />
                  )}
                </div>
              );
            })
          )}
        </div>
      </div>

      {/* ── 底部设置按钮 ─────────────────────────────────────── */}
      <div className="mt-auto border-t border-gray-200 px-3 py-3 dark:border-gray-800">
        <button
          onClick={() => navigate('/settings')}
          className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-gray-600 transition-colors hover:bg-gray-50 hover:text-gray-900 dark:text-gray-400 dark:hover:bg-gray-800/50 dark:hover:text-gray-100"
        >
          <SettingsIcon className="h-4 w-4" />
          {t('nav.settings')}
        </button>
      </div>

      {/* ── 添加项目对话框 ──────────────────────────────────── */}
      {showAddDialog && (
        <AddProjectDialog
          onClose={() => setShowAddDialog(false)}
          onAdded={(id) => {
            setShowAddDialog(false);
            navigate(`/projects/${id}`);
          }}
        />
      )}
    </aside>
  );
}

function AddProjectDialog({ onClose, onAdded }: { onClose: () => void; onAdded: (id: string) => void }) {
  const { t } = useTranslation();
  const { addProject } = useProjects();
  const [name, setName] = useState("");
  const [rootPath, setRootPath] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async () => {
    if (!name.trim() || !rootPath.trim()) return;
    setSubmitting(true);
    setError(null);
    try {
      const id = await addProject(name.trim(), rootPath.trim());
      onAdded(id);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-[400px] rounded-lg bg-white p-6 shadow-xl dark:bg-gray-900">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          {t('project.addProject')}
        </h3>
        <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
          {t('project.addProjectDesc')}
        </p>
        <div className="mt-4 space-y-3">
          <input
            type="text"
            placeholder={t('project.namePlaceholder')}
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
          />
          <input
            type="text"
            placeholder={t('project.pathPlaceholder')}
            value={rootPath}
            onChange={(e) => setRootPath(e.target.value)}
            className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
          />
        </div>
        {error && (
          <p className="mt-2 text-sm text-red-600 dark:text-red-400">{error}</p>
        )}
        <div className="mt-5 flex justify-end gap-2">
          <button
            onClick={onClose}
            className="rounded-md px-3 py-1.5 text-sm text-gray-600 hover:bg-gray-100 dark:text-gray-400 dark:hover:bg-gray-800"
          >
            {t('library.cancel')}
          </button>
          <button
            onClick={handleSubmit}
            disabled={submitting || !name.trim() || !rootPath.trim()}
            className="rounded-md bg-gray-900 px-3 py-1.5 text-sm text-white hover:bg-gray-800 disabled:opacity-50 dark:bg-gray-100 dark:text-gray-900 dark:hover:bg-gray-200"
          >
            {submitting ? "..." : t('project.addProject')}
          </button>
        </div>
      </div>
    </div>
  );
}
