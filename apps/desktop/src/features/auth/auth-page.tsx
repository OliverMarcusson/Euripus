import { zodResolver } from "@hookform/resolvers/zod";
import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Radio, Tv } from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
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
    <div className="grid min-h-screen place-items-center bg-muted/30 p-6">
      <div className="flex w-full max-w-sm flex-col gap-5">
        <div className="flex items-center gap-3">
          <div className="flex size-11 items-center justify-center rounded-2xl bg-primary text-primary-foreground">
            <Tv aria-hidden="true" />
          </div>
          <h1 className="text-2xl font-semibold tracking-tight">Welcome to Euripus</h1>
        </div>

        <Card>
          <CardHeader className="gap-3">
            <div className="flex items-center gap-3">
              <div className="flex size-10 items-center justify-center rounded-2xl bg-secondary text-secondary-foreground">
                <Radio aria-hidden="true" />
              </div>
              <CardTitle>{mode === "login" ? "Sign in" : "Create account"}</CardTitle>
            </div>
            <Tabs
              value={mode}
              onValueChange={(value) => {
                setMode(value as "login" | "register");
                form.clearErrors();
              }}
            >
              <TabsList className="grid w-full grid-cols-2">
                <TabsTrigger value="login">Login</TabsTrigger>
                <TabsTrigger value="register">Register</TabsTrigger>
              </TabsList>
            </Tabs>
          </CardHeader>

          <CardContent>
            <form className="flex flex-col gap-5" onSubmit={form.handleSubmit((values) => mutation.mutate(values))}>
              <FieldGroup>
                <Field data-invalid={form.formState.errors.username ? true : undefined}>
                  <FieldLabel htmlFor="username">Username</FieldLabel>
                  <Input
                  id="username"
                  autoComplete="username"
                  aria-invalid={form.formState.errors.username ? true : undefined}
                  {...form.register("username")}
                />
                  <FieldError errors={[form.formState.errors.username]} />
                </Field>

                <Field data-invalid={form.formState.errors.password ? true : undefined}>
                  <FieldLabel htmlFor="password">Password</FieldLabel>
                  <Input
                    id="password"
                    type="password"
                    autoComplete={mode === "login" ? "current-password" : "new-password"}
                    aria-invalid={form.formState.errors.password ? true : undefined}
                    {...form.register("password")}
                  />
                  <FieldError errors={[form.formState.errors.password]} />
                </Field>
              </FieldGroup>

              {errorMessage ? (
                <Field data-invalid>
                  <FieldError>{errorMessage}</FieldError>
                </Field>
              ) : null}

              <Button disabled={mutation.isPending} type="submit" className="w-full">
                {mutation.isPending ? "Working..." : mode === "login" ? "Login" : "Create account"}
              </Button>
            </form>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
