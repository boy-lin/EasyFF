import { Theme } from "@/components/ui/theme";
import { LanguageSwitcher } from "@/components/LanguageSwitcher";
import { UserMenu } from "@/components/auth/UserMenu";
import OnlineHelpDialog from "./OnlineHelpDialog";

export default function Header() {
  return (
    <header className="bg-background px-4 py-2 flex items-center justify-end gap-3">
      <Theme
        size="sm"
        variant="dropdown"
        themes={["light", "dark", "system"]}
        className="cursor-pointer border-transparent bg-secondary px-[9px] py-[9px] h-auto"
      />
      <LanguageSwitcher />
      <OnlineHelpDialog />
      <UserMenu />
    </header>
  );
}
