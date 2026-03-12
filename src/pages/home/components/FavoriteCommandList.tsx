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
  if (items.length === 0) {
    return <p className="text-sm text-muted-foreground">暂无收藏命令</p>;
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

