import { Outlet } from 'react-router-dom';
import UpdaterBootstrap from '@/components/app/UpdaterBootstrap';
import FavoriteSyncBootstrap from '@/components/app/FavoriteSyncBootstrap';

export default function AuthLayout() {
    return (
        <>
            <UpdaterBootstrap />
            <FavoriteSyncBootstrap />
            <Outlet />
        </>
    );
}
