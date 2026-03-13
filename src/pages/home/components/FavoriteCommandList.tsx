import { useTranslation } from "react-i18next";
import { type FavoriteCommandItem } from "@/lib/bridge";
import { FavoriteCommandCard } from "@/pages/home/components/FavoriteCommandCard";

type FavoriteCommandListProps = {
  items: FavoriteCommandItem[];
  onSelect: (item: FavoriteCommandItem) => void;
  formatUpdatedAt: (ts: number) => string;
};

export function FavoriteCommandList({
  items,
  onSelect,
  formatUpdatedAt,
}: FavoriteCommandListProps) {
  const { t } = useTranslation("ffmpeg");

  if (items.length === 0) {
    return <p className="text-sm text-muted-foreground">{t("homePage.favorites.empty")}</p>;
  }

  return (
    <div className="grid grid-cols-2 gap-2 md:grid-cols-3 lg:grid-cols-4">
      {items.map((item) => (
        <FavoriteCommandCard
          key={item.id}
          item={item}
          updatedAtText={formatUpdatedAt(item.updatedAt)}
          onSelect={onSelect}
        />
      ))}
    </div>
  );
}
