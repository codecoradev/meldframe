// The banner the page hands to the BannerStack, whichever copy of it draws (Svelte or React). A type only.

export interface Banner {
  id: string;
  tone: 'warning' | 'danger';
  text: string;
  action?: { id: string; label: string };
}
