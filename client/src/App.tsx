import { BrowserRouter, Route, Routes } from "react-router-dom";
import DashboardLayout from "./layouts/DashboardLayout";
import LoginPage from "./pages/LoginPage";
import DashboardPage from "./pages/DashboardPage";
import UsersPage from "./pages/UsersPage";
import AlbumsPage from "./pages/AlbumsPage";
import AlbumDetailPage from "./pages/AlbumDetailPage";
import PhotosPage from "./pages/PhotosLibraryPage";
import StoragePage from "./pages/StoragePage";
import RaidManagerPage from "./pages/RaidManagerPage";
import BackupManagerPage from "./pages/BackupManagerPage";
import ControlPage from "./pages/ControlPage";
import SystemUpdatesPage from "./pages/SystemUpdatesPage";
import HealthPage from "./pages/HealthPage";
import DevicesPage from "./pages/DevicesPage";
import PcBackupsPage from "./pages/PcBackupsPage";
import TvMediaPage from "./pages/TvMediaPage";
import NotificationsPage from "./pages/NotificationsPage";
import LogsPage from "./pages/LogsPage";
import SettingsPage from "./pages/SettingsPage";
import SetupPreferencesPage from "./pages/SetupPreferencesPage";
import NotFoundPage from "./pages/NotFoundPage";

export default function App(){return <BrowserRouter><Routes>
 <Route path="/" element={<LoginPage/>}/><Route path="/login" element={<LoginPage/>}/>
 <Route element={<DashboardLayout/>}>
  <Route path="/dashboard" element={<DashboardPage/>}/><Route path="/users" element={<UsersPage/>}/><Route path="/albums" element={<AlbumsPage/>}/><Route path="/albums/:id" element={<AlbumDetailPage/>}/><Route path="/photos" element={<PhotosPage/>}/>
  <Route path="/storage" element={<StoragePage/>}/><Route path="/raid" element={<RaidManagerPage/>}/><Route path="/backup" element={<BackupManagerPage/>}/><Route path="/pc-backups" element={<PcBackupsPage/>}/>
  <Route path="/notifications" element={<NotificationsPage/>}/><Route path="/logs" element={<LogsPage/>}/><Route path="/devices" element={<DevicesPage/>}/><Route path="/tv" element={<TvMediaPage/>}/>
  <Route path="/health" element={<HealthPage/>}/><Route path="/system-center" element={<HealthPage/>}/><Route path="/control" element={<ControlPage/>}/><Route path="/system" element={<SystemUpdatesPage/>}/><Route path="/updates" element={<SystemUpdatesPage/>}/><Route path="/settings" element={<SettingsPage/>}/><Route path="/settings/preferences" element={<SetupPreferencesPage/>}/><Route path="/setup/preferences" element={<SetupPreferencesPage/>}/>
 </Route><Route path="*" element={<NotFoundPage/>}/>
 </Routes></BrowserRouter>}
