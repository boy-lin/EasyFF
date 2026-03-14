import React from "react";
import { Outlet } from "react-router-dom";
import Sidebar from "./sidebar/Sidebar";
import Header from "./Header";

const Layout: React.FC = () => {
  return (
    <div
      className="flex h-screen bg-sidebar text-foreground overscroll-none"
      style={{
        overscrollBehavior: "none",
        WebkitOverflowScrolling: "touch",
      }}
    >
      <Sidebar />
      <main className="flex-1 flex flex-col overflow-hidden rounded-l-lg bg-card">
        <Header />
        <div
          className="flex-1 overflow-y-auto py-2"
          style={{
            WebkitOverflowScrolling: "touch",
            overscrollBehaviorY: "auto",
          }}
        >
          <Outlet />
        </div>
      </main>
    </div>
  );
};

export default Layout;