import type { ReactNode } from "react";

type ReceiverStatusScreenProps = {
  children?: ReactNode;
  description?: string;
  eyebrow?: string;
  notice?: string | null;
  overlay?: boolean;
  role?: "alert" | "status";
  title: string;
  tone?: "default" | "error";
};

export function ReceiverStatusScreen({
  children,
  description,
  eyebrow = "Euripus Receiver",
  notice,
  overlay = false,
  role,
  title,
  tone = "default",
}: ReceiverStatusScreenProps) {
  return (
    <div
      className={overlay ? "euripus-receiver__overlay" : "euripus-receiver"}
      data-tone={tone}
    >
      <div className="euripus-receiver__backdrop" />
      <main className="euripus-receiver__center">
        <section
          aria-live={role ? "polite" : undefined}
          className="euripus-receiver__panel"
          role={role}
        >
          <div className="euripus-receiver__message">
            <p className="euripus-receiver__eyebrow">{eyebrow}</p>
            <h1 className="euripus-receiver__title">{title}</h1>
            {description ? (
              <p className="euripus-receiver__copy">{description}</p>
            ) : null}
          </div>
          {children}
          {notice ? (
            <p className="euripus-receiver__notice">{notice}</p>
          ) : null}
        </section>
      </main>
    </div>
  );
}

export function ReceiverSpinner() {
  return <div aria-hidden="true" className="euripus-receiver__spinner" />;
}
