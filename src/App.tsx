import { Suspense } from "react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import Layout from "./components/Layout";
import Dashboard from "./pages/Dashboard";
import Library from "./pages/Library";
import SkillDetail from "./pages/SkillDetail";
import Scanner from "./pages/Scanner";
import Settings from "./pages/Settings";
import ProjectWorkspace from "./pages/ProjectWorkspace";

export default function App() {
  return (
    <Suspense fallback={<div>Loading...</div>}>
      <MemoryRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route path="/" element={<Dashboard />} />
            <Route path="/library" element={<Library />} />
            <Route path="/library/:skillName" element={<SkillDetail />} />
            <Route path="/scanner" element={<Scanner />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="/projects/:projectId" element={<ProjectWorkspace />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </Suspense>
  );
}
