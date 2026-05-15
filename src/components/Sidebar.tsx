import { NavLink, useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { LayoutDashboard, Package, Download, Settings as SettingsIcon } from "lucide-react";

const navKeys = [
  { to: "/", labelKey: "nav.dashboard", icon: LayoutDashboard },
  { to: "/library", labelKey: "nav.library", icon: Package },
  { to: "/scanner", labelKey: "nav.scanner", icon: Download },
] as const;

export default function Sidebar() {
  const { t } = useTranslation();
  const navigate = useNavigate();

  return (
    <aside className="flex w-[260px] shrink-0 flex-col border-r border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-900">
      <div className="flex h-14 items-center border-b border-gray-200 px-5 dark:border-gray-800">
        <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          {t('app.title')}
        </h1>
      </div>

      <nav className="flex-1 space-y-1 px-3 py-4">
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

      <div className="border-t border-gray-200 px-3 py-3 dark:border-gray-800">
        <button
          onClick={() => navigate('/settings')}
          className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-gray-600 transition-colors hover:bg-gray-50 hover:text-gray-900 dark:text-gray-400 dark:hover:bg-gray-800/50 dark:hover:text-gray-100"
        >
          <SettingsIcon className="h-4 w-4" />
          {t('nav.settings')}
        </button>
      </div>
    </aside>
  );
}
