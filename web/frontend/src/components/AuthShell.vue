<script setup>
// Shared shell for every IdP screen: centered card + brand header.
defineProps({
  title: { type: String, default: '' },
  subtitle: { type: String, default: '' },
})
</script>

<template>
  <div class="flex min-h-screen items-center justify-center bg-slate-100 px-4 py-10">
    <div class="w-full max-w-md">
      <div class="mb-6 flex flex-col items-center text-center">
        <div
          class="flex h-12 w-12 items-center justify-center rounded-xl bg-indigo-600 text-base font-bold text-white shadow-sm"
        >
          DH
        </div>
        <h1 class="mt-4 text-xl font-semibold text-slate-900">Digital Hub</h1>
        <p v-if="subtitle" class="mt-1 text-sm text-slate-500">{{ subtitle }}</p>
      </div>

      <div class="rounded-xl border border-slate-200 bg-white p-6 shadow-sm sm:p-8">
        <h2 v-if="title" class="text-lg font-semibold text-slate-900">{{ title }}</h2>
        <slot />
      </div>

      <p class="mt-6 text-center text-xs text-slate-400">
        Secure access by Authentik &middot; Digital Hub
      </p>
    </div>
  </div>
</template>