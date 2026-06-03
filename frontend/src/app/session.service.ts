import { computed, Injectable, signal } from "@angular/core";
import { api, type MeResponse, type User, unwrap } from "./api";

@Injectable({ providedIn: "root" })
export class SessionService {
  readonly me = signal<MeResponse | null>(null);
  readonly loading = signal(false);
  readonly error = signal<string | null>(null);
  readonly user = computed(() => this.me()?.user ?? null);

  private loadPromise: Promise<void> | null = null;

  load(): Promise<void> {
    if (this.loadPromise) {
      return this.loadPromise;
    }

    this.loading.set(true);
    this.error.set(null);
    this.loadPromise = api
      .GET("/api/me")
      .then((result) => {
        this.me.set(unwrap(result));
      })
      .catch((error: unknown) => {
        this.error.set(
          error instanceof Error ? error.message : "Login check failed",
        );
        this.me.set({ user: null });
      })
      .finally(() => {
        this.loading.set(false);
        this.loadPromise = null;
      });

    return this.loadPromise;
  }

  async refresh(): Promise<void> {
    await this.load();
  }

  setUser(user: User | null): void {
    this.me.set({ user });
    this.error.set(null);
  }

  async logout(): Promise<void> {
    await api.POST("/auth/logout");
    this.setUser(null);
  }
}
