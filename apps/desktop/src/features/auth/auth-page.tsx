import { zodResolver } from "@hookform/resolvers/zod";
import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Tv } from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";
import { ServerNetworkStatusCard } from "@/components/server/server-network-status-card";
import { Button } from "@/components/ui/button";
import { Field, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { login, register } from "@/lib/api";
import { useAuthStore } from "@/store/auth-store";

const registerSchema = z.object({
  username: z.string().min(3, "Username must be at least 3 characters.").max(64),
  password: z.string().min(8, "Password must be at least 8 characters.").max(128),
});

const loginSchema = z.object({
  username: z.string().min(1, "Enter your username.").max(64),
  password: z.string().min(1, "Enter your password.").max(128),
});

type FormValues = {
  username: string;
  password: string;
};

export function AuthPage() {
  const [mode, setMode] = useState<"login" | "register">("login");
  const navigate = useNavigate();
  const setSession = useAuthStore((state) => state.setSession);
  const form = useForm<FormValues>({
    resolver: async (values, context, options) =>
      zodResolver(mode === "login" ? loginSchema : registerSchema)(values, context, options),
    defaultValues: { username: "", password: "" },
  });

  const mutation = useMutation({
    mutationFn: async (values: FormValues) => {
      const action = mode === "login" ? login : register;
      return action(values);
    },
    onSuccess: async (session) => {
      setSession(session);
      await navigate({ to: "/guide" });
    },
  });

  const errorMessage = mutation.error instanceof Error ? mutation.error.message : null;

  return (
    <div className="container relative min-h-screen flex-col items-center justify-center grid lg:max-w-none lg:grid-cols-2 lg:px-0">
      
      {/* Left Panel: Cinematic Graphic */}
      <div className="relative hidden h-full flex-col bg-zinc-950 p-10 text-white lg:flex dark:border-r overflow-hidden shadow-[inset_0_0_100px_rgba(0,0,0,0.5)]">
        {/* Generative/Cinematic Glow Effects */}
        <div className="absolute inset-0 opacity-40">
          <div className="absolute -top-[20%] -left-[10%] size-[800px] rounded-full bg-primary/20 blur-[150px]" />
          <div className="absolute top-[60%] -right-[10%] size-[500px] rounded-full bg-fuchsia-600/20 blur-[150px]" />
          <div className="absolute top-[40%] left-[20%] size-[400px] rounded-full bg-purple-600/10 blur-[120px]" />
          {/* Subtle noise texture */}
          <div className="absolute inset-0 bg-[url('data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSI0IiBoZWlnaHQ9IjQiPjxyZWN0IHdpZHRoPSI0IiBoZWlnaHQ9IjQiIGZpbGw9IiNmZmYiIGZpbGwtb3BhY2l0eT0iMC4wMyIvPjwvc3ZnPg==')] mix-blend-overlay" />
        </div>

        <div className="relative z-20 flex items-center gap-3 text-lg font-bold">
          <div className="flex size-10 items-center justify-center rounded-2xl bg-white text-zinc-950 shadow-lg shadow-white/10">
            <Tv className="size-5" aria-hidden="true" />
          </div>
          Euripus
        </div>

      </div>

      {/* Right Panel: Sleek Form */}
      <div className="p-6 lg:p-8 h-full flex flex-col justify-center bg-background rounded-tr-3xl lg:rounded-none">
        <div className="mx-auto flex w-full flex-col justify-center space-y-8 sm:w-[400px]">
          
          {/* Mobile Header */}
          <div className="flex items-center gap-3 lg:hidden mb-4">
            <div className="flex size-10 items-center justify-center rounded-xl bg-primary text-primary-foreground">
              <Tv className="size-5" aria-hidden="true" />
            </div>
            <h1 className="text-xl font-bold tracking-tight">Euripus</h1>
          </div>

          <div className="flex flex-col space-y-2 text-left">
            <h1 className="text-3xl font-bold tracking-tight">
              {mode === "login" ? "Welcome back" : "Create an account"}
            </h1>
          </div>

          <div className="grid gap-6">
            <Tabs
              value={mode}
              onValueChange={(value) => {
                setMode(value as "login" | "register");
                form.clearErrors();
              }}
              className="w-full"
            >
              <TabsList className="grid w-full grid-cols-2 p-1 bg-muted/50 rounded-xl">
                <TabsTrigger className="rounded-lg transition-all data-[state=active]:bg-background data-[state=active]:shadow-sm" value="login">Login</TabsTrigger>
                <TabsTrigger className="rounded-lg transition-all data-[state=active]:bg-background data-[state=active]:shadow-sm" value="register">Register</TabsTrigger>
              </TabsList>
            </Tabs>

            <form className="flex flex-col gap-5 mt-2" onSubmit={form.handleSubmit((values) => mutation.mutate(values))}>
              <FieldGroup className="gap-5">
                <Field data-invalid={form.formState.errors.username ? true : undefined}>
                  <FieldLabel htmlFor="username" className="text-xs uppercase tracking-wider text-muted-foreground font-semibold">Username</FieldLabel>
                  <Input
                    id="username"
                    autoComplete="username"
                    className="h-12 bg-transparent border-b-2 border-border/50 border-t-0 border-l-0 border-r-0 rounded-none px-0 focus-visible:ring-0 focus-visible:border-primary transition-colors text-lg font-medium placeholder:font-normal placeholder:text-muted-foreground/50"
                    placeholder="e.g. director_steven"
                    aria-invalid={form.formState.errors.username ? true : undefined}
                    {...form.register("username")}
                  />
                  <FieldError errors={[form.formState.errors.username]} className="mt-1" />
                </Field>

                <Field data-invalid={form.formState.errors.password ? true : undefined}>
                  <FieldLabel htmlFor="password" className="text-xs uppercase tracking-wider text-muted-foreground font-semibold">Password</FieldLabel>
                  <Input
                    id="password"
                    type="password"
                    autoComplete={mode === "login" ? "current-password" : "new-password"}
                    className="h-12 bg-transparent border-b-2 border-border/50 border-t-0 border-l-0 border-r-0 rounded-none px-0 focus-visible:ring-0 focus-visible:border-primary transition-colors text-lg font-medium placeholder:font-normal placeholder:text-muted-foreground/50"
                    placeholder="••••••••"
                    aria-invalid={form.formState.errors.password ? true : undefined}
                    {...form.register("password")}
                  />
                  <FieldError errors={[form.formState.errors.password]} className="mt-1" />
                </Field>
              </FieldGroup>

              {errorMessage ? (
                <Field data-invalid>
                  <FieldError className="p-3 bg-destructive/10 rounded-xl border border-destructive/20 text-destructive">{errorMessage}</FieldError>
                </Field>
              ) : null}

              <Button 
                disabled={mutation.isPending} 
                type="submit" 
                className="w-full h-12 text-base rounded-xl mt-4 font-semibold hover:scale-[1.02] transition-transform active:scale-95"
              >
                {mutation.isPending ? "Authenticating..." : mode === "login" ? "Sign In" : "Create account"}
              </Button>
            </form>

            <ServerNetworkStatusCard
              className="border-border/70 bg-muted/20"
            />
          </div>
        </div>
      </div>
    </div>
  );
}
