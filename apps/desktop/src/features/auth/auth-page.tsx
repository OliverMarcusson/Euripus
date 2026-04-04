import { zodResolver } from "@hookform/resolvers/zod";
import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";
import { login, register } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
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
    <div className="grid min-h-screen place-items-center p-6">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>Welcome to Euripus</CardTitle>
          <CardDescription>Sign in to sync favorites, provider credentials, and playback continuity across devices.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-5">
          <div className="flex gap-2 rounded-xl bg-secondary/60 p-1">
            <Button
              className="flex-1"
              variant={mode === "login" ? "default" : "ghost"}
              onClick={() => {
                setMode("login");
                form.clearErrors();
              }}
              type="button"
            >
              Login
            </Button>
            <Button
              className="flex-1"
              variant={mode === "register" ? "default" : "ghost"}
              onClick={() => {
                setMode("register");
                form.clearErrors();
              }}
              type="button"
            >
              Register
            </Button>
          </div>
          <form className="flex flex-col gap-4" onSubmit={form.handleSubmit((values) => mutation.mutate(values))}>
            <div className="flex flex-col gap-2">
              <label className="text-sm font-medium" htmlFor="username">
                Username
              </label>
              <Input id="username" autoComplete="username" {...form.register("username")} />
              {form.formState.errors.username ? (
                <p className="text-sm text-destructive">{form.formState.errors.username.message}</p>
              ) : null}
            </div>
            <div className="flex flex-col gap-2">
              <label className="text-sm font-medium" htmlFor="password">
                Password
              </label>
              <Input id="password" type="password" autoComplete={mode === "login" ? "current-password" : "new-password"} {...form.register("password")} />
              {form.formState.errors.password ? (
                <p className="text-sm text-destructive">{form.formState.errors.password.message}</p>
              ) : null}
            </div>
            {errorMessage ? <p className="text-sm text-destructive">{errorMessage}</p> : null}
            <Button disabled={mutation.isPending} type="submit">
              {mutation.isPending ? "Working..." : mode === "login" ? "Login" : "Create account"}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
