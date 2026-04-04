import { PageHeader } from "@/components/layout/page-header";
import { ProviderSettingsSection } from "@/features/provider/provider-settings-section";

export function ProviderPage() {
  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Provider" />
      <ProviderSettingsSection />
    </div>
  );
}
