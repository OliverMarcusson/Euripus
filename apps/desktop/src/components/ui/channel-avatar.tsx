import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { cn } from "@/lib/utils";

type ChannelAvatarProps = {
  name: string;
  logoUrl?: string | null;
  className?: string;
  fallbackClassName?: string;
};

export function ChannelAvatar({ name, logoUrl, className, fallbackClassName }: ChannelAvatarProps) {
  return (
    <Avatar className={cn("size-11 rounded-2xl", className)}>
      {logoUrl ? <AvatarImage src={logoUrl} alt={`${name} logo`} className="object-cover" /> : null}
      <AvatarFallback
        className={cn(
          "rounded-2xl bg-secondary text-sm font-semibold text-secondary-foreground",
          fallbackClassName,
        )}
      >
        {getInitials(name)}
      </AvatarFallback>
    </Avatar>
  );
}

function getInitials(name: string) {
  const initials = name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("");

  return initials || name.slice(0, 2).toUpperCase();
}
