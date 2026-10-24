<template>
  <div class="logo">
    <a target="_blank" href="https://shizengzhou.github.io/motr/">
      <svg
        xmlns="http://www.w3.org/2000/svg"
        :width="width"
        :height="height"
        viewBox="0 0 58 14"
      >
        <g fill-rule="evenodd">
          <path
            d="M42,0 L56,0 C57,0 58,1 58,2 L58,7.5 C58,8.5 57,9.5 56,9.5 L54.2,9.5 L58,14 L53,14 L49.2,9.5 L45,9.5 C44.5,9.5 44,10 44,10.5 L44,14 L40,14 L40,2 C40,1 41,0 42,0 Z M44,3.5 C44,3 44.5,2.5 45,2.5 L53,2.5 C53.5,2.5 54,3 54,3.5 L54,6 C54,6.5 53.5,7 53,7 L45,7 C44.5,7 44,6.5 44,6 Z"
          />
          <rect width="4" height="2" x="32" y="6" rx=".5" />
          <path
            d="M2,0 L26,0 C27,-2.04003481e-15 28,1 28,2 L28,14 L24,14 L24,3.5 C24,3 23.5,2.5 23,2.5 L16,2.5 L16,14 L12,14 L12,2.5 L5,2.5 C4.5,2.5 4,3 4,3.5 L4,14 L0,14 L0,2 C0,1 1,-2.04003481e-15 2,0 Z"
          />
        </g>
      </svg>
    </a>
  </div>
</template>

<script>
export default {
  name: 'mo-logo',
  props: {
    width: {
      type: Number,
      default: 58,
    },
    height: {
      type: Number,
      default: 14,
    },
  },
};
</script>

<style lang="scss">
.logo {
  display: inline-block;
  margin: 0;
  padding: 0;
  > a {
    display: block;
    width: 100%;
    height: 100%;
    outline: none;
    border: none;
    text-align: center;
    font-size: 0;
    color: $--app-logo-color;
  }
  svg {
    fill: currentColor;
  }
}
</style>
