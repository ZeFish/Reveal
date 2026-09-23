<script>
  /** @type {{ class?: string }} */
  let { class: className = "" } = $props();
</script>

<div class="grain-bg {className}">
  <div class="grain-overlay"></div>
</div>

<style>
  .grain-bg {
    position: absolute;
    inset: 0;
    background-color: var(--color-surface-lowest);
    overflow: hidden;
    pointer-events: none;
    z-index: -1;
  }

  .grain-overlay {
    position: absolute;
    inset: -150%;
    width: 300%;
    height: 300%;
    background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 200 200' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='noiseFilter'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.8' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noiseFilter)' opacity='0.035'/%3E%3C/svg%3E");
    opacity: 0.85;
    animation: grain-animation 8s steps(10) infinite;
  }

  @keyframes grain-animation {
    0%, 100% { transform: translate(0, 0); }
    10% { transform: translate(-5%, -10%); }
    20% { transform: translate(-15%, 5%); }
    30% { transform: translate(7%, -25%); }
    40% { transform: translate(-5%, 25%); }
    50% { transform: translate(-15%, 10%); }
    60% { transform: translate(15%, 0); }
    70% { transform: translate(0, 15%); }
    80% { transform: translate(3%, 35%); }
    90% { transform: translate(-10%, 10%); }
  }
</style>
