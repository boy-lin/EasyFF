import { EllipsisName } from "@/components/ui-lab/ellipsis-name";
import { type FavoriteCommandItem } from "@/lib/bridge";

type FavoriteCommandCardProps = {
  item: FavoriteCommandItem;
  updatedAtText: string;
  onSelect: (item: FavoriteCommandItem) => void;
};

export function FavoriteCommandCard({ item, updatedAtText, onSelect }: FavoriteCommandCardProps) {
  return (
    <article
      className="cursor-pointer rounded-lg border bg-muted/25 p-3 transition-colors hover:bg-muted/40"
      onClick={() => onSelect(item)}
    >
      <p className="text-sm font-semibold">
        <EllipsisName name={item.title} startCount={10} endCount={10} />
      </p>
      <p className="mt-1 font-mono text-sm">
        <EllipsisName name={item.command} startCount={20} />
      </p>
      {item.description && <p className="mt-2 text-xs text-muted-foreground">{item.description}</p>}
      <p className="mt-2 text-xs text-muted-foreground">更新于: {updatedAtText}</p>
    </article>
  );
}

