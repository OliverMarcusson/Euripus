import type {
  AdminRestrictedAccountInput,
  AdminPatternGroup,
  AdminPatternGroupInput,
  AdminPatternKind,
  AdminPatternGroupImportInput,
} from "@euripus/shared";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown } from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { PageHeader } from "@/components/layout/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  adminLogin,
  adminLogout,
  createAdminRestrictedAccount,
  deleteAdminRestrictedAccount,
  createAdminPatternGroup,
  deleteAllAdminPatternGroups,
  deleteAdminPatternGroup,
  getAdminPatternGroups,
  getAdminQualityChannelPrefixes,
  saveAdminQualityChannelPrefixes,
  getAdminRestrictedAccounts,
  getAdminImportErrors,
  importAdminPatternGroups,
  testAdminSearchQuery,
  testAdminSearchPatterns,
  updateAdminPatternGroup,
  updateAdminRestrictedAccount,
} from "@/lib/api";

type EditableGroup = AdminPatternGroupInput & {
  id?: string;
};

const DEFAULT_GROUP: EditableGroup = {
  kind: "country",
  value: "",
  matchTarget: "channel_or_category",
  matchMode: "prefix",
  priority: 0,
  enabled: true,
  patternsText: "",
  countryCodesText: "",
};

const DEFAULT_JSON_IMPORT = `[
  {
    "kind": "country",
    "value": "se",
    "matchTarget": "channel_or_category",
    "matchMode": "prefix",
    "priority": 10,
    "enabled": true,
    "patterns": ["SE:", "SE|", "SWE|", "SWEDEN"]
  }
]`;

const DEFAULT_MANAGED_ACCOUNT: AdminRestrictedAccountInput = {
  username: "",
  password: "",
  provider: {
    baseUrl: "",
    username: "",
    password: "",
    outputFormat: "m3u8",
    playbackMode: "direct",
    epgSources: [],
  },
};

export function AdminPage() {
  const queryClient = useQueryClient();
  const [password, setPassword] = useState("");
  const [selectedQualityPrefixes, setSelectedQualityPrefixes] = useState<string[]>([]);
  const [includeCategoriesWithoutCountryPrefix, setIncludeCategoriesWithoutCountryPrefix] = useState(false);
  const [loginError, setLoginError] = useState<string | null>(null);
  const [draftGroup, setDraftGroup] = useState<EditableGroup>(DEFAULT_GROUP);
  const [testInput, setTestInput] = useState({
    channelName: "",
    categoryName: "",
    programTitle: "",
  });
  const [isJsonImportOpen, setIsJsonImportOpen] = useState(false);
  const [jsonImportValue, setJsonImportValue] = useState(DEFAULT_JSON_IMPORT);
  const [jsonImportParseError, setJsonImportParseError] = useState<string | null>(null);
  const [searchQueryInput, setSearchQueryInput] = useState(
    "country:se provider:viaplay !ppv epg",
  );
  const [managedAccount, setManagedAccount] = useState<AdminRestrictedAccountInput>(DEFAULT_MANAGED_ACCOUNT);
  const [editingManagedAccountId, setEditingManagedAccountId] = useState<string | null>(null);

  const groupsQuery = useQuery({
    queryKey: ["admin", "pattern-groups"],
    queryFn: getAdminPatternGroups,
    retry: false,
  });
  const qualityPrefixesQuery = useQuery({
    queryKey: ["admin", "quality-channel-prefixes"],
    queryFn: getAdminQualityChannelPrefixes,
    retry: false,
  });
  const managedAccountsQuery = useQuery({
    queryKey: ["admin", "restricted-accounts"],
    queryFn: getAdminRestrictedAccounts,
    retry: false,
  });

  useEffect(() => {
    if (qualityPrefixesQuery.data) {
      setSelectedQualityPrefixes(qualityPrefixesQuery.data.prefixes.filter((entry) => entry.selected).map((entry) => entry.prefix));
      setIncludeCategoriesWithoutCountryPrefix(qualityPrefixesQuery.data.includeCategoriesWithoutCountryPrefix);
    }
  }, [qualityPrefixesQuery.data]);

  const saveQualityPrefixesMutation = useMutation({
    mutationFn: saveAdminQualityChannelPrefixes,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["admin", "quality-channel-prefixes"] });
    },
  });

  const loginMutation = useMutation({
    mutationFn: adminLogin,
    onSuccess: async () => {
      setLoginError(null);
      setPassword("");
      await queryClient.invalidateQueries({ queryKey: ["admin"] });
    },
    onError: (error) => {
      setLoginError(error instanceof Error ? error.message : "Unable to sign in");
    },
  });

  const logoutMutation = useMutation({
    mutationFn: adminLogout,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["admin"] });
    },
  });
  const createManagedAccountMutation = useMutation({
    mutationFn: createAdminRestrictedAccount,
    onSuccess: async () => {
      setManagedAccount(DEFAULT_MANAGED_ACCOUNT);
      await queryClient.invalidateQueries({ queryKey: ["admin", "restricted-accounts"] });
    },
  });
  const deleteManagedAccountMutation = useMutation({
    mutationFn: deleteAdminRestrictedAccount,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["admin", "restricted-accounts"] });
    },
  });
  const updateManagedAccountMutation = useMutation({
    mutationFn: ({ id, payload }: { id: string; payload: AdminRestrictedAccountInput }) => updateAdminRestrictedAccount(id, payload),
    onSuccess: async () => {
      setManagedAccount(DEFAULT_MANAGED_ACCOUNT);
      setEditingManagedAccountId(null);
      await queryClient.invalidateQueries({ queryKey: ["admin", "restricted-accounts"] });
    },
  });

  const createMutation = useMutation({
    mutationFn: createAdminPatternGroup,
    onSuccess: async () => {
      setDraftGroup(DEFAULT_GROUP);
      await queryClient.invalidateQueries({ queryKey: ["admin", "pattern-groups"] });
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, payload }: { id: string; payload: AdminPatternGroupInput }) =>
      updateAdminPatternGroup(id, payload),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["admin", "pattern-groups"] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteAdminPatternGroup,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["admin", "pattern-groups"] });
    },
  });

  const deleteAllMutation = useMutation({
    mutationFn: deleteAllAdminPatternGroups,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["admin", "pattern-groups"] });
    },
  });

  const importMutation = useMutation({
    mutationFn: importAdminPatternGroups,
    onSuccess: async () => {
      setIsJsonImportOpen(false);
      setJsonImportValue(DEFAULT_JSON_IMPORT);
      setJsonImportParseError(null);
      await queryClient.invalidateQueries({ queryKey: ["admin", "pattern-groups"] });
    },
  });

  const testMutation = useMutation({
    mutationFn: testAdminSearchPatterns,
  });

  const queryTestMutation = useMutation({
    mutationFn: testAdminSearchQuery,
  });

  const grouped = useMemo(() => {
    const groups = groupsQuery.data ?? [];
    return {
      country: groups.filter((group) => group.kind === "country"),
      provider: groups.filter((group) => group.kind === "provider"),
      flag: groups.filter((group) => group.kind === "flag"),
    };
  }, [groupsQuery.data]);

  const unauthorized =
    groupsQuery.isError &&
    groupsQuery.error instanceof Error &&
    /authentication is required/i.test(groupsQuery.error.message);

  if (groupsQuery.isLoading) {
    return <div className="grid min-h-screen place-items-center">Loading admin panel...</div>;
  }

  if (unauthorized) {
    return (
      <div className="mx-auto flex min-h-screen w-full max-w-md items-center px-4 py-12">
        <Card className="w-full">
          <CardHeader>
            <CardTitle>Admin login</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="admin-password">Password</Label>
              <Input
                id="admin-password"
                type="password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    loginMutation.mutate(password);
                  }
                }}
              />
            </div>
            {loginError ? <p className="text-sm text-destructive">{loginError}</p> : null}
            <Button
              onClick={() => loginMutation.mutate(password)}
              disabled={loginMutation.isPending || password.trim().length === 0}
            >
              {loginMutation.isPending ? "Signing in..." : "Sign in"}
            </Button>
          </CardContent>
        </Card>
      </div>
    );
  }

  if (groupsQuery.isError) {
    return (
      <div className="grid min-h-screen place-items-center px-4 text-center">
        <p>{groupsQuery.error instanceof Error ? groupsQuery.error.message : "Unable to load admin panel"}</p>
      </div>
    );
  }

  return (
    <div className="mx-auto flex w-full max-w-6xl flex-col gap-6 px-4 py-6">
      <PageHeader
        title="Search Admin"
        meta={
          <>
            <Badge variant="outline">{(groupsQuery.data ?? []).length} groups</Badge>
            <Button
              variant="outline"
              size="sm"
              onClick={() => logoutMutation.mutate()}
              disabled={logoutMutation.isPending}
            >
              Sign out
            </Button>
          </>
        }
      />

      <Card>
        <CardHeader><CardTitle>Quality channels</CardTitle></CardHeader>
        <CardContent className="flex flex-col gap-4">
          <p className="text-sm text-muted-foreground">Choose the country prefixes considered quality channels. Prefixes are discovered from channel and category names in the database.</p>
          <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
            {(qualityPrefixesQuery.data?.prefixes ?? []).map((entry) => (
              <label key={entry.prefix} className="flex items-center gap-3 rounded-lg border p-3">
                <input type="checkbox" checked={selectedQualityPrefixes.includes(entry.prefix)} onChange={(event) => setSelectedQualityPrefixes((current) => event.target.checked ? [...current, entry.prefix] : current.filter((prefix) => prefix !== entry.prefix))} />
                <span><strong>{entry.prefix}</strong><span className="block text-xs text-muted-foreground">{entry.channelCount} channels · {entry.categoryCount} categories</span></span>
              </label>
            ))}
          </div>
          {!qualityPrefixesQuery.isPending && !(qualityPrefixesQuery.data?.prefixes.length) ? <p className="text-sm text-muted-foreground">No prefixes were found.</p> : null}
          <label className="flex items-center gap-3 rounded-lg border p-3">
            <input type="checkbox" checked={includeCategoriesWithoutCountryPrefix} onChange={(event) => setIncludeCategoriesWithoutCountryPrefix(event.target.checked)} />
            <span><strong>Categories without a country prefix</strong><span className="block text-xs text-muted-foreground">Include channels in categories that do not start with a letter-only country prefix.</span></span>
          </label>
          {qualityPrefixesQuery.isError ? <p className="text-sm text-destructive">Unable to load quality prefixes.</p> : null}
          <Button onClick={() => saveQualityPrefixesMutation.mutate({ prefixes: selectedQualityPrefixes, includeCategoriesWithoutCountryPrefix })} disabled={saveQualityPrefixesMutation.isPending}>
            {saveQualityPrefixesMutation.isPending ? "Saving..." : "Save quality prefixes"}
          </Button>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Managed restricted accounts</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <p className="text-sm text-muted-foreground">These users can sync their assigned provider but cannot change its settings.</p>
          <div className="grid gap-3 md:grid-cols-2">
            <Input aria-label="Managed account username" placeholder="Account username" value={managedAccount.username} onChange={(event) => setManagedAccount((value) => ({ ...value, username: event.target.value }))} />
            <Input aria-label="Managed account password" type="password" placeholder="Login password" value={managedAccount.password ?? ""} onChange={(event) => setManagedAccount((value) => ({ ...value, password: event.target.value }))} />
            <Input aria-label="Managed provider URL" placeholder="Provider URL" value={managedAccount.provider.baseUrl} onChange={(event) => setManagedAccount((value) => ({ ...value, provider: { ...value.provider, baseUrl: event.target.value } }))} />
            <Input aria-label="Managed provider username" placeholder="Provider username" value={managedAccount.provider.username} onChange={(event) => setManagedAccount((value) => ({ ...value, provider: { ...value.provider, username: event.target.value } }))} />
            <Input aria-label="Managed provider password" type="password" placeholder="Provider password" value={managedAccount.provider.password} onChange={(event) => setManagedAccount((value) => ({ ...value, provider: { ...value.provider, password: event.target.value } }))} />
            <Input aria-label="Managed EPG URLs" placeholder="EPG URLs (space-separated)" value={managedAccount.provider.epgSources.map((source) => source.url).join(" ")} onChange={(event) => setManagedAccount((value) => ({ ...value, provider: { ...value.provider, epgSources: event.target.value.split(/\s+/).filter(Boolean).map((url, priority) => ({ url, priority, enabled: true })) } }))} />
            <select aria-label="Managed output format" className="h-9 rounded-md border bg-background px-3" value={managedAccount.provider.outputFormat} onChange={(event) => setManagedAccount((value) => ({ ...value, provider: { ...value.provider, outputFormat: event.target.value as "m3u8" | "ts" } }))}><option value="m3u8">M3U8</option><option value="ts">TS</option></select>
            <select aria-label="Managed playback mode" className="h-9 rounded-md border bg-background px-3" value={managedAccount.provider.playbackMode} onChange={(event) => setManagedAccount((value) => ({ ...value, provider: { ...value.provider, playbackMode: event.target.value as "direct" | "relay" } }))}><option value="direct">Direct</option><option value="relay">Relay</option></select>
          </div>
          {createManagedAccountMutation.error instanceof Error ? <p className="text-sm text-destructive">{createManagedAccountMutation.error.message}</p> : null}
          <Button onClick={() => editingManagedAccountId ? updateManagedAccountMutation.mutate({ id: editingManagedAccountId, payload: managedAccount }) : createManagedAccountMutation.mutate(managedAccount)} disabled={createManagedAccountMutation.isPending || updateManagedAccountMutation.isPending}>
            {createManagedAccountMutation.isPending || updateManagedAccountMutation.isPending ? "Saving..." : editingManagedAccountId ? "Save managed account" : "Create managed account"}
          </Button>
          <div className="flex flex-col gap-2">
            {(managedAccountsQuery.data ?? []).map((account) => (
              <div key={account.id} className="flex items-center justify-between gap-3 rounded-lg border p-3">
                <div className="min-w-0"><p className="font-medium">{account.username}</p><p className="text-xs text-muted-foreground">{account.providerStatus ?? "No provider"}{account.providerLastSyncAt ? ` · synced ${new Date(account.providerLastSyncAt).toLocaleString()}` : ""}</p></div>
                <div className="flex gap-2"><Button variant="outline" size="sm" onClick={() => { setEditingManagedAccountId(account.id); setManagedAccount({ username: account.username, password: "", provider: { baseUrl: account.providerBaseUrl ?? "", username: account.providerUsername ?? "", password: "", outputFormat: account.providerOutputFormat ?? "m3u8", playbackMode: account.providerPlaybackMode ?? "direct", epgSources: account.providerEpgUrls.map((url, priority) => ({ url, priority, enabled: true })) } }); }}>Edit</Button><Button variant="destructive" size="sm" onClick={() => deleteManagedAccountMutation.mutate(account.id)} disabled={deleteManagedAccountMutation.isPending}>Delete</Button></div>
              </div>
            ))}
            {managedAccountsQuery.isError ? <p className="text-sm text-destructive">Unable to load managed accounts.</p> : null}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between gap-3">
            <CardTitle>Create pattern group</CardTitle>
            <div className="flex items-center gap-2">
              <Button variant="outline" onClick={() => setIsJsonImportOpen(true)}>
                Add JSON
              </Button>
              <Button
                variant="outline"
                onClick={() => {
                  if ((groupsQuery.data?.length ?? 0) === 0) {
                    return;
                  }

                  const confirmed = window.confirm(
                    "Delete all search rules? This will remove every country, provider, and flag rule.",
                  );
                  if (!confirmed) {
                    return;
                  }

                  deleteAllMutation.mutate();
                }}
                disabled={deleteAllMutation.isPending || (groupsQuery.data?.length ?? 0) === 0}
              >
                {deleteAllMutation.isPending ? "Deleting..." : "Delete all"}
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <PatternGroupForm
            value={draftGroup}
            submitLabel={createMutation.isPending ? "Saving..." : "Create group"}
            onChange={setDraftGroup}
            onSubmit={(payload) => createMutation.mutate(payload)}
          />
        </CardContent>
      </Card>

      {isJsonImportOpen ? (
        <JsonImportModal
          value={jsonImportValue}
          parseError={jsonImportParseError}
          importErrors={getAdminImportErrors(importMutation.error)}
          pending={importMutation.isPending}
          onChange={(nextValue) => {
            setJsonImportValue(nextValue);
            setJsonImportParseError(null);
            importMutation.reset();
          }}
          onClose={() => {
            setIsJsonImportOpen(false);
            setJsonImportParseError(null);
            importMutation.reset();
          }}
          onSubmit={() => {
            const parsed = parseJsonImportValue(jsonImportValue);
            if (!parsed.ok) {
              setJsonImportParseError(parsed.message);
              return;
            }

            setJsonImportParseError(null);
            importMutation.mutate({ groups: parsed.groups });
          }}
        />
      ) : null}

      <section className="grid gap-6 lg:grid-cols-3">
        <PatternGroupList
          title="Countries"
          groups={grouped.country}
          onSave={(id, payload) => updateMutation.mutate({ id, payload })}
          onDelete={(id) => deleteMutation.mutate(id)}
          pending={updateMutation.isPending || deleteMutation.isPending}
        />
        <PatternGroupList
          title="Providers"
          groups={grouped.provider}
          onSave={(id, payload) => updateMutation.mutate({ id, payload })}
          onDelete={(id) => deleteMutation.mutate(id)}
          pending={updateMutation.isPending || deleteMutation.isPending}
        />
        <PatternGroupList
          title="Flags"
          groups={grouped.flag}
          onSave={(id, payload) => updateMutation.mutate({ id, payload })}
          onDelete={(id) => deleteMutation.mutate(id)}
          pending={updateMutation.isPending || deleteMutation.isPending}
        />
      </section>

      <Card>
        <CardHeader>
          <CardTitle>Test patterns</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-4 lg:grid-cols-2">
          <div className="grid gap-4">
            <div className="grid gap-2">
              <Label htmlFor="test-channel">Channel name</Label>
              <Input
                id="test-channel"
                value={testInput.channelName}
                onChange={(event) =>
                  setTestInput((current) => ({ ...current, channelName: event.target.value }))
                }
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="test-category">Category name</Label>
              <Input
                id="test-category"
                value={testInput.categoryName}
                onChange={(event) =>
                  setTestInput((current) => ({ ...current, categoryName: event.target.value }))
                }
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="test-program">Program title</Label>
              <Input
                id="test-program"
                value={testInput.programTitle}
                onChange={(event) =>
                  setTestInput((current) => ({ ...current, programTitle: event.target.value }))
                }
              />
            </div>
            <Button
              onClick={() =>
                testMutation.mutate({
                  channelName: testInput.channelName || null,
                  categoryName: testInput.categoryName || null,
                  programTitle: testInput.programTitle || null,
                })
              }
              disabled={testMutation.isPending}
            >
              {testMutation.isPending ? "Testing..." : "Run test"}
            </Button>
          </div>

          <div className="grid gap-3 rounded-xl border border-border p-4">
            <ResultRow label="Country" value={testMutation.data?.countryCode ?? "None"} />
            <ResultRow label="Provider" value={testMutation.data?.providerName ?? "None"} />
            <ResultRow label="PPV" value={testMutation.data?.isPpv ? "Yes" : "No"} />
            <ResultRow label="VIP" value={testMutation.data?.isVip ? "Yes" : "No"} />
            <ResultRow
              label="Force EPG"
              value={testMutation.data?.forceHasEpg ? "Yes" : "No"}
            />
            {testMutation.isError ? (
              <p className="text-sm text-destructive">
                {testMutation.error instanceof Error
                  ? testMutation.error.message
                  : "Unable to run test"}
              </p>
            ) : null}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Test search query</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-4 lg:grid-cols-2">
          <div className="grid gap-4">
            <div className="grid gap-2">
              <Label htmlFor="test-search-query">Search query</Label>
              <Input
                id="test-search-query"
                value={searchQueryInput}
                onChange={(event) => setSearchQueryInput(event.target.value)}
                placeholder="country:se provider:viaplay !ppv epg"
              />
            </div>
            <Button
              onClick={() => queryTestMutation.mutate({ query: searchQueryInput })}
              disabled={queryTestMutation.isPending || searchQueryInput.trim().length === 0}
            >
              {queryTestMutation.isPending ? "Testing..." : "Run search test"}
            </Button>
          </div>

          <div className="grid gap-3 rounded-xl border border-border p-4">
            <ResultRow
              label="Free text"
              value={queryTestMutation.data?.search || "None"}
            />
            <ResultRow
              label="Country"
              value={formatListValue(queryTestMutation.data?.countries)}
            />
            <ResultRow
              label="Provider"
              value={formatListValue(queryTestMutation.data?.providers)}
            />
            <ResultRow
              label="PPV"
              value={formatOptionalBoolean(queryTestMutation.data?.ppv)}
            />
            <ResultRow
              label="VIP"
              value={formatOptionalBoolean(queryTestMutation.data?.vip)}
            />
            <ResultRow
              label="EPG"
              value={queryTestMutation.data?.requireEpg ? "Yes" : "No"}
            />
            {queryTestMutation.isError ? (
              <p className="text-sm text-destructive">
                {queryTestMutation.error instanceof Error
                  ? queryTestMutation.error.message
                  : "Unable to run search test"}
              </p>
            ) : null}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function PatternGroupList({
  title,
  groups,
  onSave,
  onDelete,
  pending,
}: {
  title: string;
  groups: AdminPatternGroup[];
  onSave: (id: string, payload: AdminPatternGroupInput) => void;
  onDelete: (id: string) => void;
  pending: boolean;
}) {
  const [isExpanded, setIsExpanded] = useState(false);

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-center gap-0 py-4">
        <button
          type="button"
          className="flex w-full items-center justify-between gap-3 px-0 py-0 text-left"
          onClick={() => setIsExpanded((current) => !current)}
          aria-expanded={isExpanded}
          aria-label={`${isExpanded ? "Collapse" : "Expand"} ${title}`}
        >
          <CardTitle className="text-base">{title}</CardTitle>
          <div className="flex items-center self-center gap-2 pr-1">
            <Badge
              variant="outline"
              className="inline-flex min-w-8 items-center justify-center px-2"
            >
              {groups.length}
            </Badge>
            <span className="flex items-center text-sm leading-none text-muted-foreground">
              {isExpanded ? "Hide" : "Show"}
            </span>
            <ChevronDown
              className={`h-4 w-4 self-center text-muted-foreground transition-transform ${
                isExpanded ? "rotate-180" : ""
              }`}
            />
          </div>
        </button>
      </CardHeader>
      {isExpanded ? (
        <CardContent className="flex flex-col gap-4">
          {groups.length ? (
            groups.map((group) => (
              <PatternGroupSummaryCard
                key={group.id}
                group={group}
                onSave={onSave}
                onDelete={onDelete}
                pending={pending}
              />
            ))
          ) : (
            <p className="text-sm text-muted-foreground">No groups yet.</p>
          )}
        </CardContent>
      ) : null}
    </Card>
  );
}

function PatternGroupSummaryCard({
  group,
  onSave,
  onDelete,
  pending,
}: {
  group: AdminPatternGroup;
  onSave: (id: string, payload: AdminPatternGroupInput) => void;
  onDelete: (id: string) => void;
  pending: boolean;
}) {
  const [isEditing, setIsEditing] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const [value, setValue] = useState<EditableGroup>({
    kind: group.kind,
    value: group.value,
    matchTarget: group.matchTarget,
    matchMode: group.matchMode,
    priority: group.priority,
    enabled: group.enabled,
    patternsText: group.patternsText,
    countryCodesText: group.countryCodesText,
    id: group.id,
  });

  useEffect(() => {
    setValue({
      kind: group.kind,
      value: group.value,
      matchTarget: group.matchTarget,
      matchMode: group.matchMode,
      priority: group.priority,
      enabled: group.enabled,
      patternsText: group.patternsText,
      countryCodesText: group.countryCodesText,
      id: group.id,
    });
  }, [group]);

  return (
    <>
      <div className="rounded-xl border border-border p-4">
        <div className="flex items-start justify-between gap-3">
          <div
            role="button"
            tabIndex={0}
            className="grid flex-1 gap-2 text-left"
            onClick={() => setIsExpanded((current) => !current)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                setIsExpanded((current) => !current);
              }
            }}
            aria-expanded={isExpanded}
            aria-label={`${isExpanded ? "Collapse" : "Expand"} ${group.value}`}
          >
            <div className="flex flex-wrap items-center gap-2">
              <span className="font-medium">{group.value}</span>
              <Badge variant={group.enabled ? "default" : "outline"}>
                {group.enabled ? "Enabled" : "Disabled"}
              </Badge>
              <span className="text-xs text-muted-foreground">
                {isExpanded ? "Hide details" : "Show details"}
              </span>
            </div>
            <p className={isExpanded ? "text-sm text-muted-foreground" : "hidden"}>
              {formatKindLabel(group.kind)} • {formatMatchTargetLabel(group.matchTarget)} •{" "}
              {formatMatchModeLabel(group.matchMode)} • Priority {group.priority}
            </p>
            <p
              className={
                isExpanded ? "text-sm text-muted-foreground break-words" : "hidden"
              }
            >
              {group.patternsText}
            </p>
            {group.kind === "provider" ? (
              <p className={isExpanded ? "text-sm text-muted-foreground" : "hidden"}>
                Countries: {group.countryCodesText || "None"}
              </p>
            ) : null}
          </div>

          <Button
            variant="outline"
            size="sm"
            onClick={() => setIsEditing(true)}
            aria-label={`Edit ${group.value}`}
          >
            Edit
          </Button>
        </div>
      </div>

      {isEditing ? (
        <EditPatternGroupModal
          value={value}
          pending={pending}
          onChange={setValue}
          onClose={() => setIsEditing(false)}
          onDelete={() => {
            onDelete(group.id);
            setIsEditing(false);
          }}
          onSubmit={(payload) => {
            onSave(group.id, payload);
            setIsEditing(false);
          }}
        />
      ) : null}
    </>
  );
}

function EditPatternGroupModal({
  value,
  pending,
  onChange,
  onClose,
  onDelete,
  onSubmit,
}: {
  value: EditableGroup;
  pending: boolean;
  onChange: (value: EditableGroup) => void;
  onClose: () => void;
  onDelete: () => void;
  onSubmit: (payload: AdminPatternGroupInput) => void;
}) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 px-4 py-8"
      onClick={onClose}
    >
      <div
        className="max-h-[90vh] w-full max-w-2xl overflow-y-auto rounded-2xl border border-border bg-background p-6 shadow-2xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="mb-4 flex items-center justify-between gap-3">
          <div>
            <h2 className="text-lg font-semibold">Edit pattern group</h2>
            <p className="text-sm text-muted-foreground">
              Update the value, matching behavior, and patterns for this rule.
            </p>
          </div>
          <Button variant="outline" size="sm" onClick={onClose}>
            Close
          </Button>
        </div>

        <PatternGroupForm
          value={value}
          submitLabel={pending ? "Saving..." : "Save changes"}
          onChange={onChange}
          onSubmit={onSubmit}
          secondaryAction={
            <Button variant="outline" onClick={onDelete} disabled={pending}>
              Delete
            </Button>
          }
        />
      </div>
    </div>
  );
}

function JsonImportModal({
  value,
  parseError,
  importErrors,
  pending,
  onChange,
  onClose,
  onSubmit,
}: {
  value: string;
  parseError: string | null;
  importErrors: Array<{ index: number; field: string; message: string }>;
  pending: boolean;
  onChange: (value: string) => void;
  onClose: () => void;
  onSubmit: () => void;
}) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 px-4 py-8"
      onClick={onClose}
    >
      <div
        className="max-h-[90vh] w-full max-w-3xl overflow-y-auto rounded-2xl border border-border bg-background p-6 shadow-2xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="mb-4 flex items-center justify-between gap-3">
          <div>
            <h2 className="text-lg font-semibold">Import pattern groups from JSON</h2>
            <p className="text-sm text-muted-foreground">
              Paste a top-level JSON array. See <code>docs/admin-search-rule-json.md</code> for
              the canonical format.
            </p>
          </div>
          <Button variant="outline" size="sm" onClick={onClose}>
            Close
          </Button>
        </div>

        <div className="grid gap-4">
          <div className="grid gap-2">
            <Label htmlFor="json-import">JSON input</Label>
            <textarea
              id="json-import"
              className="min-h-[320px] rounded-md border border-input bg-background px-3 py-3 font-mono text-sm"
              value={value}
              onChange={(event) => onChange(event.target.value)}
              spellCheck={false}
            />
          </div>

          {parseError ? <p className="text-sm text-destructive">{parseError}</p> : null}

          {importErrors.length ? (
            <div className="rounded-xl border border-destructive/40 bg-destructive/5 p-4">
              <p className="mb-2 text-sm font-medium text-destructive">Import validation errors</p>
              <div className="grid gap-1">
                {importErrors.map((error) => (
                  <p
                    key={`${error.index}-${error.field}-${error.message}`}
                    className="text-sm text-destructive"
                  >
                    Item {error.index + 1}, <span className="font-medium">{error.field}</span>:{" "}
                    {error.message}
                  </p>
                ))}
              </div>
            </div>
          ) : null}

          <div className="flex flex-wrap gap-2">
            <Button onClick={onSubmit} disabled={pending}>
              {pending ? "Importing..." : "Import rules"}
            </Button>
            <Button variant="outline" onClick={onClose} disabled={pending}>
              Cancel
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}

function PatternGroupForm({
  value,
  onChange,
  onSubmit,
  submitLabel,
  secondaryAction,
}: {
  value: EditableGroup;
  onChange: (value: EditableGroup) => void;
  onSubmit: (payload: AdminPatternGroupInput) => void;
  submitLabel: string;
  secondaryAction?: ReactNode;
}) {
  return (
    <div className="grid gap-4">
      <div className="grid gap-2">
        <Label>Kind</Label>
        <select
          className="h-10 rounded-md border border-input bg-background px-3 text-sm"
          value={value.kind}
          onChange={(event) =>
            onChange({ ...value, kind: event.target.value as AdminPatternKind })
          }
        >
          <option value="country">Country</option>
          <option value="provider">Provider</option>
          <option value="flag">Flag</option>
        </select>
      </div>

      <div className="grid gap-2">
        <Label>Value</Label>
        <Input
          value={value.value}
          onChange={(event) => onChange({ ...value, value: event.target.value })}
          placeholder="SE or viaplay or ppv"
        />
      </div>

      <div className="grid gap-2">
        <Label>Match target</Label>
        <select
          className="h-10 rounded-md border border-input bg-background px-3 text-sm"
          value={value.matchTarget}
          onChange={(event) =>
            onChange({ ...value, matchTarget: event.target.value as EditableGroup["matchTarget"] })
          }
        >
          <option value="channel_name">Channel name</option>
          <option value="category_name">Category name</option>
          <option value="program_title">Program title</option>
          <option value="channel_or_category">Channel or category</option>
          <option value="any_text">Any text</option>
        </select>
      </div>

      <div className="grid gap-2">
        <Label>Match mode</Label>
        <select
          className="h-10 rounded-md border border-input bg-background px-3 text-sm"
          value={value.matchMode}
          onChange={(event) =>
            onChange({ ...value, matchMode: event.target.value as EditableGroup["matchMode"] })
          }
        >
          <option value="prefix">Prefix</option>
          <option value="contains">Contains</option>
          <option value="exact">Exact</option>
        </select>
      </div>

      <div className="grid gap-2">
        <Label>Patterns</Label>
        <Input
          value={value.patternsText}
          onChange={(event) => onChange({ ...value, patternsText: event.target.value })}
          placeholder="SE:,SE|"
        />
      </div>

      {value.kind === "provider" ? (
        <div className="grid gap-2">
          <Label>Related countries</Label>
          <Input
            value={value.countryCodesText}
            onChange={(event) =>
              onChange({ ...value, countryCodesText: event.target.value })
            }
            placeholder="se,uk"
          />
        </div>
      ) : null}

      <div className="grid gap-2">
        <Label>Priority</Label>
        <Input
          type="number"
          value={value.priority}
          onChange={(event) =>
            onChange({ ...value, priority: Number(event.target.value) || 0 })
          }
        />
      </div>

      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={value.enabled}
          onChange={(event) => onChange({ ...value, enabled: event.target.checked })}
        />
        Enabled
      </label>

      <div className="flex flex-wrap gap-2">
        <Button
          onClick={() =>
            onSubmit({
              kind: value.kind,
              value: value.value,
              matchTarget: value.matchTarget,
              matchMode: value.matchMode,
              priority: value.priority,
              enabled: value.enabled,
              patternsText: value.patternsText,
              countryCodesText: value.countryCodesText,
            })
          }
        >
          {submitLabel}
        </Button>
        {secondaryAction}
      </div>
    </div>
  );
}

function ResultRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <span className="text-sm text-muted-foreground">{label}</span>
      <span className="font-medium">{value}</span>
    </div>
  );
}

function formatListValue(values: string[] | undefined) {
  if (!values?.length) {
    return "None";
  }

  return values.join(", ");
}

function formatOptionalBoolean(value: boolean | null | undefined) {
  if (value === true) {
    return "Yes";
  }
  if (value === false) {
    return "No";
  }
  return "Not set";
}

function formatKindLabel(value: AdminPatternKind) {
  switch (value) {
    case "country":
      return "Country";
    case "provider":
      return "Provider";
    case "flag":
      return "Flag";
  }
}

function formatMatchTargetLabel(value: EditableGroup["matchTarget"]) {
  switch (value) {
    case "channel_name":
      return "Channel name";
    case "category_name":
      return "Category name";
    case "program_title":
      return "Program title";
    case "channel_or_category":
      return "Channel or category";
    case "any_text":
      return "Any text";
  }
}

function formatMatchModeLabel(value: EditableGroup["matchMode"]) {
  switch (value) {
    case "prefix":
      return "Prefix";
    case "contains":
      return "Contains";
    case "exact":
      return "Exact";
  }
}

function parseJsonImportValue(
  value: string,
): { ok: true; groups: AdminPatternGroupImportInput[] } | { ok: false; message: string } {
  try {
    const parsed = JSON.parse(value) as unknown;
    if (!Array.isArray(parsed)) {
      return {
        ok: false,
        message: "JSON import must be a top-level array of pattern-group objects.",
      };
    }

    return {
      ok: true,
      groups: parsed as AdminPatternGroupImportInput[],
    };
  } catch (error) {
    return {
      ok: false,
      message: error instanceof Error ? error.message : "Invalid JSON input.",
    };
  }
}
