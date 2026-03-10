<script lang="ts">
  let { navigate }: { navigate: (to: string) => void } = $props();
</script>

<nav>
  <button class="logo" onclick={() => navigate('/')}>Relay</button>
</nav>

<style>
  nav {
    display: flex;
    align-items: center;
    padding: 0 2rem;
    height: 64px;
    border-bottom: 1px solid var(--border);
    background: var(--bg);
  }

  .logo {
    font-size: 1.6rem;
    font-weight: 700;
    color: var(--blue);
    background: none;
    border: none;
    border-radius: 0;
    padding: 0;
    cursor: pointer;
    letter-spacing: -0.03em;
  }

  .logo:hover {
    opacity: 0.8;
  }
</style>
