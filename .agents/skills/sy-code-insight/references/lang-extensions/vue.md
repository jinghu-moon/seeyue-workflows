# Lang Extension: Vue 3

Load when: `package.json` exists AND `.vue` files detected.

## Additional Interface Fields

```json
{
  "props": {
    "paramName": { "type": "string", "required": true, "default": null }
  },
  "emits": ["update:modelValue", "save", "cancel"],
  "slots": ["default", "header"],
  "expose": ["reset", "validate"]
}
```

## Vue-Specific Constraints

- MUST NOT mutate props directly
- MUST NOT call async inside `computed`
- MUST emit `update:modelValue` for v-model contract
- Props down, Events up — no two-way prop mutation

## Reactivity Strategy

- **Source**: `ref<T>(initial)` — keep minimal
- **Derived**: `computed(() => ...)` — derive everything possible
- **Effects**: `watch(source, handler)` — async side effects only
- MUST NOT recompute expensive logic in templates

## Data Flow Pattern

```
Parent
  ↓ props (user, mode)
Component
  ↑ emits (save, cancel)
Parent
```

## Composition API Rules

- `defineProps<T>()` for type-safe props
- `defineEmits<T>()` for type-safe events
- `defineExpose()` for public API
- Composables: `use*` naming convention

## Quality Checklist (Vue-Specific)

- [ ] Single responsibility in one sentence?
- [ ] Data flow pattern declared?
- [ ] Reactivity strategy explicit?
- [ ] Props/emits typed with `defineProps`/`defineEmits`?
- [ ] Enum values documented with behavior?
