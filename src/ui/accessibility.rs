//! Native UI Automation tree for the owner-data process table.
//!
//! The fragment is hosted by the table HWND, so native toolbar and inspector
//! controls continue using their Windows providers. Rectangles are screen
//! coordinates; the UI supplies complete labelled values, including unavailable
//! and partial-coverage explanations. Children are related by stable identities,
//! rather than by their indentation or their current display indices.

#![allow(non_snake_case, unsafe_op_in_unsafe_fn)]

use std::{
    collections::{HashMap, VecDeque},
    ffi::c_void,
    ptr::{null_mut, write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};
use windows::{
    Win32::{
        Foundation::{E_ACCESSDENIED, E_FAIL, E_NOINTERFACE, E_OUTOFMEMORY, E_POINTER, S_OK},
        System::{
            Com::SAFEARRAY,
            Ole::{SafeArrayCreateVector, SafeArrayDestroy, SafeArrayPutElement},
            Variant::{VARIANT, VT_I4, VT_UNKNOWN},
        },
        UI::Accessibility::*,
    },
    core::{BOOL, BSTR, GUID, HRESULT, IUnknown, IUnknown_Vtbl, Interface, PCWSTR},
};
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    UI::{
        Input::KeyboardAndMouse::{GetFocus, SetFocus},
        WindowsAndMessaging::{GetParent, GetWindowRect, PostMessageW, WM_APP},
    },
};

#[derive(Clone)]
pub(super) struct AccessibleRow {
    pub id: String,
    pub parent: Option<String>,
    pub name: String,
    pub value: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    pub selected: bool,
    pub rect: RECT,
}

#[derive(Clone)]
pub(super) struct AccessibleTable {
    pub rows: Vec<AccessibleRow>,
    pub sort: String,
    pub focused: Option<String>,
    pub hwnd: HWND,
}

// HWND is an opaque OS identifier, never a dereferenced application pointer.
// The data is copied under its mutex; all mutations are posted to the UI thread.
unsafe impl Send for AccessibleTable {}

struct Context {
    state: Arc<Mutex<AccessibleTable>>,
    hwnd: usize,
    owner: usize,
    alive: AtomicBool,
    runtime_ids: Mutex<HashMap<String, i32>>,
    actions: Mutex<VecDeque<(usize, String)>>,
}

pub(super) struct AccessibilityProvider {
    root: IRawElementProviderSimple,
    context: Arc<Context>,
    previous: Mutex<Option<AccessibleTable>>,
}

pub(super) fn install(hwnd: HWND, state: Arc<Mutex<AccessibleTable>>) -> AccessibilityProvider {
    let context = Arc::new(Context {
        state,
        hwnd: hwnd as usize,
        owner: unsafe { GetParent(hwnd) } as usize,
        alive: AtomicBool::new(true),
        runtime_ids: Mutex::new(HashMap::new()),
        actions: Mutex::new(VecDeque::new()),
    });
    AccessibilityProvider {
        root: unsafe { IRawElementProviderSimple::from_raw(create(context.clone(), None, SIMPLE)) },
        context,
        previous: Mutex::new(None),
    }
}

impl AccessibilityProvider {
    pub(super) fn handle_wm_getobject(&self, wparam: usize, lparam: isize) -> Option<isize> {
        (lparam as i32 == UiaRootObjectId).then(|| unsafe {
            UiaReturnRawElementProvider(
                windows::Win32::Foundation::HWND(self.context.hwnd as *mut c_void),
                windows::Win32::Foundation::WPARAM(wparam),
                windows::Win32::Foundation::LPARAM(lparam),
                &self.root,
            )
            .0
        })
    }

    /// Drain on WM_APP+20 and resolve the stable identity against the current
    /// model. A posted display index could otherwise select a different process.
    pub(super) fn take_action(&self) -> Option<(usize, String)> {
        self.context.actions.lock().ok()?.pop_front()
    }

    pub(super) fn notify_changed(&self) {
        let Ok(current) = self.context.state.lock().map(|state| state.clone()) else {
            return;
        };
        let Ok(mut previous) = self.previous.lock() else {
            return;
        };
        unsafe {
            let changed_structure = previous.as_ref().is_none_or(|old| {
                old.rows
                    .iter()
                    .map(|row| (&row.id, &row.parent))
                    .ne(current.rows.iter().map(|row| (&row.id, &row.parent)))
            });
            if changed_structure {
                let _ = UiaRaiseStructureChangedEvent(
                    &self.root,
                    StructureChangeType_ChildrenInvalidated,
                    null_mut(),
                    0,
                );
            }
            if previous.as_ref().is_none_or(|old| old.sort != current.sort) {
                let old =
                    VARIANT::from(previous.as_ref().map(|old| old.sort.as_str()).unwrap_or(""));
                let new = VARIANT::from(current.sort.as_str());
                let _ = UiaRaiseAutomationPropertyChangedEvent(
                    &self.root,
                    UIA_ItemStatusPropertyId,
                    &old,
                    &new,
                );
            }
            let old_rows: HashMap<_, _> = previous
                .as_ref()
                .map(|old| old.rows.iter().map(|row| (row.id.as_str(), row)).collect())
                .unwrap_or_default();
            for row in &current.rows {
                let old = old_rows.get(row.id.as_str()).copied();
                let expanded_changed = old.is_some_and(|old| old.expanded != row.expanded);
                let newly_selected = row.selected && old.is_none_or(|old| !old.selected);
                let value_changed = old.is_some_and(|old| old.value != row.value);
                if !expanded_changed && !newly_selected && !value_changed {
                    continue;
                }
                let provider = IRawElementProviderSimple::from_raw(create(
                    self.context.clone(),
                    Some(row.id.clone()),
                    SIMPLE,
                ));
                if expanded_changed {
                    let prior = VARIANT::from(expansion(old.unwrap()).0);
                    let next = VARIANT::from(expansion(row).0);
                    let _ = UiaRaiseAutomationPropertyChangedEvent(
                        &provider,
                        UIA_ExpandCollapseExpandCollapseStatePropertyId,
                        &prior,
                        &next,
                    );
                }
                if newly_selected {
                    let _ = UiaRaiseAutomationEvent(
                        &provider,
                        UIA_SelectionItem_ElementSelectedEventId,
                    );
                }
                if value_changed {
                    let prior = VARIANT::from(old.unwrap().value.as_str());
                    let next = VARIANT::from(row.value.as_str());
                    let _ = UiaRaiseAutomationPropertyChangedEvent(
                        &provider,
                        UIA_ValueValuePropertyId,
                        &prior,
                        &next,
                    );
                }
            }
            if previous
                .as_ref()
                .is_none_or(|old| old.focused != current.focused)
                && GetFocus() == self.context.hwnd as HWND
            {
                let focused = IRawElementProviderSimple::from_raw(create(
                    self.context.clone(),
                    current.focused.clone(),
                    SIMPLE,
                ));
                let _ = UiaRaiseAutomationEvent(&focused, UIA_AutomationFocusChangedEventId);
            }
        }
        *previous = Some(current);
    }
}

impl Drop for AccessibilityProvider {
    fn drop(&mut self) {
        self.context.alive.store(false, Ordering::Release);
        unsafe {
            let _ = UiaDisconnectProvider(&self.root);
        }
    }
}

const SIMPLE: usize = 0;
const FRAGMENT: usize = 1;
const ROOT: usize = 2;
const SELECT_ITEM: usize = 3;
const EXPAND: usize = 4;
const VALUE: usize = 5;
const SELECTION: usize = 6;
const UNAVAILABLE: HRESULT = HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32);
const INVALID_OPERATION: HRESULT = HRESULT(UIA_E_INVALIDOPERATION as i32);

// Explicit COM ABI thunks make nullable results unambiguous: unsupported
// patterns and absent neighbours MUST return S_OK and write a null out-pointer.
// The windows-rs projected Result<Interface> cannot express that result.
#[repr(C)]
struct Node {
    slots: [*const c_void; 7],
    references: AtomicU32,
    context: Arc<Context>,
    id: Option<String>,
}

fn create(context: Arc<Context>, id: Option<String>, slot: usize) -> *mut c_void {
    let node = Box::new(Node {
        slots: [
            &SIMPLE_VT as *const _ as _,
            &FRAGMENT_VT as *const _ as _,
            &ROOT_VT as *const _ as _,
            &SELECT_VT as *const _ as _,
            &EXPAND_VT as *const _ as _,
            &VALUE_VT as *const _ as _,
            &SELECTION_VT as *const _ as _,
        ],
        references: AtomicU32::new(1),
        context,
        id,
    });
    unsafe { Box::into_raw(node).cast::<*const c_void>().add(slot).cast() }
}

unsafe fn node<'a, const SLOT: usize>(this: *mut c_void) -> &'a Node {
    &*this.cast::<*const c_void>().sub(SLOT).cast::<Node>()
}

impl Node {
    fn row(&self) -> Result<Option<AccessibleRow>, HRESULT> {
        if !self.context.alive.load(Ordering::Acquire) {
            return Err(UNAVAILABLE);
        }
        let state = self.context.state.lock().map_err(|_| E_FAIL)?;
        match &self.id {
            Some(id) => state
                .rows
                .iter()
                .find(|row| &row.id == id)
                .cloned()
                .map(Some)
                .ok_or(UNAVAILABLE),
            None => Ok(None),
        }
    }
    fn post(&self, action: usize) -> HRESULT {
        let row = match self.row() {
            Ok(Some(row)) => row,
            Ok(None) => return INVALID_OPERATION,
            Err(error) => return error,
        };
        if (action == 2 || action == 3) && !row.has_children {
            return INVALID_OPERATION;
        }
        let Ok(mut actions) = self.context.actions.lock() else {
            return E_FAIL;
        };
        actions.push_back((action, row.id));
        if unsafe { PostMessageW(self.context.owner as HWND, WM_APP + 20, action, 0) } == 0 {
            actions.pop_back();
            return E_FAIL;
        }
        S_OK
    }
}

unsafe extern "system" fn query<const SLOT: usize>(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    write(out, null_mut());
    if iid.is_null() {
        return E_POINTER;
    }
    let object = node::<SLOT>(this);
    let slot = if *iid == IUnknown::IID || *iid == IRawElementProviderSimple::IID {
        SIMPLE
    } else if *iid == IRawElementProviderFragment::IID {
        FRAGMENT
    } else if *iid == IRawElementProviderFragmentRoot::IID && object.id.is_none() {
        ROOT
    } else if *iid == ISelectionProvider::IID && object.id.is_none() {
        SELECTION
    } else if *iid == ISelectionItemProvider::IID && object.id.is_some() {
        SELECT_ITEM
    } else if *iid == IExpandCollapseProvider::IID && object.id.is_some() {
        EXPAND
    } else if *iid == IValueProvider::IID {
        VALUE
    } else {
        return E_NOINTERFACE;
    };
    object.references.fetch_add(1, Ordering::Relaxed);
    write(
        out,
        (object as *const Node as *mut *const c_void)
            .add(slot)
            .cast(),
    );
    S_OK
}
unsafe extern "system" fn add_ref<const SLOT: usize>(this: *mut c_void) -> u32 {
    node::<SLOT>(this)
        .references
        .fetch_add(1, Ordering::Relaxed)
        + 1
}
unsafe extern "system" fn release<const SLOT: usize>(this: *mut c_void) -> u32 {
    let object = node::<SLOT>(this);
    let remaining = object.references.fetch_sub(1, Ordering::Release) - 1;
    if remaining == 0 {
        std::sync::atomic::fence(Ordering::Acquire);
        drop(Box::from_raw(object as *const Node as *mut Node));
    }
    remaining
}
const fn unknown<const SLOT: usize>() -> IUnknown_Vtbl {
    IUnknown_Vtbl {
        QueryInterface: query::<SLOT>,
        AddRef: add_ref::<SLOT>,
        Release: release::<SLOT>,
    }
}

unsafe fn output<T>(out: *mut T, value: T) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    write(out, value);
    S_OK
}
unsafe extern "system" fn options(_: *mut c_void, out: *mut ProviderOptions) -> HRESULT {
    output(
        out,
        ProviderOptions_ServerSideProvider | ProviderOptions_ProviderOwnsSetFocus,
    )
}
unsafe extern "system" fn pattern(
    this: *mut c_void,
    pattern: UIA_PATTERN_ID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    write(out, null_mut());
    let object = node::<SIMPLE>(this);
    let row = match object.row() {
        Ok(row) => row,
        Err(error) => return error,
    };
    let slot = if pattern == UIA_ValuePatternId {
        VALUE
    } else if pattern == UIA_SelectionPatternId && row.is_none() {
        SELECTION
    } else if pattern == UIA_SelectionItemPatternId && row.is_some() {
        SELECT_ITEM
    } else if pattern == UIA_ExpandCollapsePatternId
        && row.as_ref().is_some_and(|row| row.has_children)
    {
        EXPAND
    } else {
        return S_OK;
    };
    object.references.fetch_add(1, Ordering::Relaxed);
    write(
        out,
        (object as *const Node as *mut *const c_void)
            .add(slot)
            .cast(),
    );
    S_OK
}

fn expansion(row: &AccessibleRow) -> ExpandCollapseState {
    if !row.has_children {
        ExpandCollapseState_LeafNode
    } else if row.expanded {
        ExpandCollapseState_Expanded
    } else {
        ExpandCollapseState_Collapsed
    }
}

// These are SDK constants imported from windows-rs, with their official names.
#[allow(non_upper_case_globals)]
unsafe extern "system" fn property(
    this: *mut c_void,
    property: UIA_PROPERTY_ID,
    out: *mut VARIANT,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    write(out, VARIANT::default());
    let object = node::<SIMPLE>(this);
    let row = match object.row() {
        Ok(row) => row,
        Err(error) => return error,
    };
    let Ok(state) = object.context.state.lock() else {
        return E_FAIL;
    };
    let value = match property {
        UIA_ControlTypePropertyId => VARIANT::from(if row.is_some() {
            UIA_TreeItemControlTypeId.0
        } else {
            UIA_TreeControlTypeId.0
        }),
        UIA_NamePropertyId => VARIANT::from(
            row.as_ref()
                .map(|row| row.name.as_str())
                .unwrap_or("Processes"),
        ),
        UIA_AutomationIdPropertyId => {
            VARIANT::from(object.id.as_deref().unwrap_or("MemMap.ProcessTree"))
        }
        UIA_HelpTextPropertyId | UIA_ValueValuePropertyId => VARIANT::from(
            row.as_ref()
                .map(|row| row.value.as_str())
                .unwrap_or(state.sort.as_str()),
        ),
        UIA_ItemStatusPropertyId => VARIANT::from(state.sort.as_str()),
        UIA_IsKeyboardFocusablePropertyId
        | UIA_IsEnabledPropertyId
        | UIA_IsControlElementPropertyId
        | UIA_IsContentElementPropertyId
        | UIA_ValueIsReadOnlyPropertyId => VARIANT::from(true),
        UIA_HasKeyboardFocusPropertyId => {
            VARIANT::from(GetFocus() == object.context.hwnd as HWND && object.id == state.focused)
        }
        UIA_SelectionItemIsSelectedPropertyId if row.is_some() => {
            VARIANT::from(row.as_ref().unwrap().selected)
        }
        UIA_ExpandCollapseExpandCollapseStatePropertyId if row.is_some() => {
            VARIANT::from(expansion(row.as_ref().unwrap()).0)
        }
        UIA_LevelPropertyId if row.is_some() => {
            VARIANT::from(row.as_ref().unwrap().depth as i32 + 1)
        }
        UIA_PositionInSetPropertyId | UIA_SizeOfSetPropertyId if row.is_some() => {
            let row = row.as_ref().unwrap();
            let siblings: Vec<_> = state
                .rows
                .iter()
                .filter(|other| other.parent == row.parent)
                .collect();
            VARIANT::from(if property == UIA_SizeOfSetPropertyId {
                siblings.len() as i32
            } else {
                siblings
                    .iter()
                    .position(|other| other.id == row.id)
                    .unwrap_or(0) as i32
                    + 1
            })
        }
        UIA_IsOffscreenPropertyId if row.is_some() => {
            let r = row.as_ref().unwrap().rect;
            let mut bounds = std::mem::zeroed();
            let known = GetWindowRect(object.context.hwnd as HWND, &mut bounds) != 0;
            VARIANT::from(
                !known
                    || r.right <= r.left
                    || r.bottom <= r.top
                    || r.bottom <= bounds.top
                    || r.top >= bounds.bottom
                    || r.right <= bounds.left
                    || r.left >= bounds.right,
            )
        }
        _ => VARIANT::default(),
    };
    write(out, value);
    S_OK
}
unsafe extern "system" fn host(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    write(out, null_mut());
    let object = node::<SIMPLE>(this);
    if !object.context.alive.load(Ordering::Acquire) {
        return UNAVAILABLE;
    }
    if object.id.is_some() {
        return S_OK;
    }
    match UiaHostProviderFromHwnd(windows::Win32::Foundation::HWND(
        object.context.hwnd as *mut c_void,
    )) {
        Ok(provider) => {
            write(out, provider.into_raw());
            S_OK
        }
        Err(error) => error.code(),
    }
}

// Preserve the SDK NavigateDirection names in the COM dispatch match.
#[allow(non_upper_case_globals)]
unsafe extern "system" fn navigate(
    this: *mut c_void,
    direction: NavigateDirection,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    write(out, null_mut());
    let object = node::<FRAGMENT>(this);
    let row = match object.row() {
        Ok(row) => row,
        Err(error) => return error,
    };
    let Ok(state) = object.context.state.lock() else {
        return E_FAIL;
    };
    let target: Option<Option<String>> = match direction {
        NavigateDirection_Parent => row.as_ref().map(|row| row.parent.clone()),
        NavigateDirection_FirstChild | NavigateDirection_LastChild => {
            if row.as_ref().is_some_and(|row| !row.expanded) {
                None
            } else {
                let mut children = state.rows.iter().filter(|row| row.parent == object.id);
                (if direction == NavigateDirection_FirstChild {
                    children.next()
                } else {
                    children.next_back()
                })
                .map(|row| Some(row.id.clone()))
            }
        }
        NavigateDirection_NextSibling | NavigateDirection_PreviousSibling if row.is_some() => {
            let row = row.as_ref().unwrap();
            let siblings: Vec<_> = state
                .rows
                .iter()
                .filter(|other| other.parent == row.parent)
                .collect();
            siblings
                .iter()
                .position(|other| other.id == row.id)
                .and_then(|index| {
                    if direction == NavigateDirection_NextSibling {
                        siblings.get(index + 1)
                    } else {
                        index.checked_sub(1).and_then(|index| siblings.get(index))
                    }
                })
                .map(|row| Some(row.id.clone()))
        }
        _ => None,
    };
    if let Some(id) = target {
        write(out, create(object.context.clone(), id, FRAGMENT));
    }
    S_OK
}
unsafe extern "system" fn runtime_id(this: *mut c_void, out: *mut *mut SAFEARRAY) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    write(out, null_mut());
    let object = node::<FRAGMENT>(this);
    if let Err(error) = object.row() {
        return error;
    }
    let Some(id) = &object.id else { return S_OK };
    let Ok(mut ids) = object.context.runtime_ids.lock() else {
        return E_FAIL;
    };
    let next = ids.len() as i32 + 1;
    let number = *ids.entry(id.clone()).or_insert(next);
    let array = SafeArrayCreateVector(VT_I4, 0, 2);
    if array.is_null() {
        return E_OUTOFMEMORY;
    }
    for (index, value) in [UiaAppendRuntimeId as i32, number].iter().enumerate() {
        if let Err(error) =
            SafeArrayPutElement(array, &(index as i32), (value as *const i32).cast())
        {
            let _ = SafeArrayDestroy(array);
            return error.code();
        }
    }
    write(out, array);
    S_OK
}
unsafe extern "system" fn bounds(this: *mut c_void, out: *mut UiaRect) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    write(out, UiaRect::default());
    let object = node::<FRAGMENT>(this);
    let row = match object.row() {
        Ok(row) => row,
        Err(error) => return error,
    };
    let mut r: RECT = std::mem::zeroed();
    if let Some(row) = row {
        r = row.rect;
    } else if GetWindowRect(object.context.hwnd as HWND, &mut r) == 0 {
        return E_FAIL;
    }
    output(
        out,
        UiaRect {
            left: r.left as f64,
            top: r.top as f64,
            width: (r.right - r.left).max(0) as f64,
            height: (r.bottom - r.top).max(0) as f64,
        },
    )
}
unsafe extern "system" fn embedded(_: *mut c_void, out: *mut *mut SAFEARRAY) -> HRESULT {
    output(out, null_mut())
}
unsafe extern "system" fn focus(this: *mut c_void) -> HRESULT {
    let object = node::<FRAGMENT>(this);
    if let Err(error) = object.row() {
        return error;
    }
    if object.id.is_some() {
        object.post(1)
    } else {
        SetFocus(object.context.hwnd as HWND);
        S_OK
    }
}
unsafe extern "system" fn fragment_root(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    write(out, null_mut());
    let object = node::<FRAGMENT>(this);
    if let Err(error) = object.row() {
        return error;
    }
    output(out, create(object.context.clone(), None, ROOT))
}
unsafe extern "system" fn from_point(
    this: *mut c_void,
    x: f64,
    y: f64,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    write(out, null_mut());
    let object = node::<ROOT>(this);
    if let Err(error) = object.row() {
        return error;
    }
    let Ok(state) = object.context.state.lock() else {
        return E_FAIL;
    };
    let id = state
        .rows
        .iter()
        .rev()
        .find(|row| {
            x >= row.rect.left as f64
                && x < row.rect.right as f64
                && y >= row.rect.top as f64
                && y < row.rect.bottom as f64
        })
        .map(|row| row.id.clone());
    let mut r = std::mem::zeroed();
    if id.is_none()
        && (GetWindowRect(object.context.hwnd as HWND, &mut r) == 0
            || x < r.left as f64
            || x >= r.right as f64
            || y < r.top as f64
            || y >= r.bottom as f64)
    {
        return S_OK;
    }
    output(out, create(object.context.clone(), id, FRAGMENT))
}
unsafe extern "system" fn get_focus(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    write(out, null_mut());
    let object = node::<ROOT>(this);
    if let Err(error) = object.row() {
        return error;
    }
    if GetFocus() != object.context.hwnd as HWND {
        return S_OK;
    }
    let Ok(state) = object.context.state.lock() else {
        return E_FAIL;
    };
    output(
        out,
        create(object.context.clone(), state.focused.clone(), FRAGMENT),
    )
}
unsafe extern "system" fn select(this: *mut c_void) -> HRESULT {
    node::<SELECT_ITEM>(this).post(1)
}
unsafe extern "system" fn add_selection(this: *mut c_void) -> HRESULT {
    let object = node::<SELECT_ITEM>(this);
    if let Err(error) = object.row() {
        return error;
    }
    {
        let Ok(state) = object.context.state.lock() else {
            return E_FAIL;
        };
        if state
            .rows
            .iter()
            .any(|row| row.selected && Some(&row.id) != object.id.as_ref())
        {
            return INVALID_OPERATION;
        }
    }
    object.post(1)
}
unsafe extern "system" fn remove_selection(this: *mut c_void) -> HRESULT {
    match node::<SELECT_ITEM>(this).row() {
        Ok(Some(row)) if !row.selected => S_OK,
        Ok(_) => INVALID_OPERATION,
        Err(error) => error,
    }
}
unsafe extern "system" fn selected(this: *mut c_void, out: *mut BOOL) -> HRESULT {
    match node::<SELECT_ITEM>(this).row() {
        Ok(Some(row)) => output(out, BOOL::from(row.selected)),
        Ok(None) => INVALID_OPERATION,
        Err(error) => error,
    }
}
unsafe extern "system" fn selection_container(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    write(out, null_mut());
    let object = node::<SELECT_ITEM>(this);
    if let Err(error) = object.row() {
        return error;
    }
    output(out, create(object.context.clone(), None, SIMPLE))
}
unsafe extern "system" fn expand(this: *mut c_void) -> HRESULT {
    node::<EXPAND>(this).post(2)
}
unsafe extern "system" fn collapse(this: *mut c_void) -> HRESULT {
    node::<EXPAND>(this).post(3)
}
unsafe extern "system" fn expansion_state(
    this: *mut c_void,
    out: *mut ExpandCollapseState,
) -> HRESULT {
    match node::<EXPAND>(this).row() {
        Ok(Some(row)) => output(out, expansion(&row)),
        Ok(None) => INVALID_OPERATION,
        Err(error) => error,
    }
}
unsafe extern "system" fn set_value(_: *mut c_void, _: PCWSTR) -> HRESULT {
    E_ACCESSDENIED
}
unsafe extern "system" fn value(this: *mut c_void, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    write(out, null_mut());
    let object = node::<VALUE>(this);
    let text = match object.row() {
        Ok(Some(row)) => row.value,
        Ok(None) => match object.context.state.lock() {
            Ok(state) => state.sort.clone(),
            Err(_) => return E_FAIL,
        },
        Err(error) => return error,
    };
    output(out, BSTR::from(text.as_str()).into_raw().cast_mut().cast())
}
unsafe extern "system" fn read_only(_: *mut c_void, out: *mut BOOL) -> HRESULT {
    output(out, BOOL(1))
}
unsafe extern "system" fn false_value(_: *mut c_void, out: *mut BOOL) -> HRESULT {
    output(out, BOOL(0))
}
unsafe extern "system" fn selection_required(this: *mut c_void, out: *mut BOOL) -> HRESULT {
    let object = node::<SELECTION>(this);
    if let Err(error) = object.row() {
        return error;
    }
    let Ok(state) = object.context.state.lock() else {
        return E_FAIL;
    };
    output(out, BOOL::from(state.rows.iter().any(|row| row.selected)))
}
unsafe extern "system" fn get_selection(this: *mut c_void, out: *mut *mut SAFEARRAY) -> HRESULT {
    if out.is_null() {
        return E_POINTER;
    }
    write(out, null_mut());
    let object = node::<SELECTION>(this);
    if let Err(error) = object.row() {
        return error;
    }
    let Ok(state) = object.context.state.lock() else {
        return E_FAIL;
    };
    let selected = state.rows.iter().find(|row| row.selected);
    let array = SafeArrayCreateVector(VT_UNKNOWN, 0, u32::from(selected.is_some()));
    if array.is_null() {
        return E_OUTOFMEMORY;
    }
    if let Some(row) = selected {
        let provider = IRawElementProviderSimple::from_raw(create(
            object.context.clone(),
            Some(row.id.clone()),
            SIMPLE,
        ));
        if let Err(error) = SafeArrayPutElement(array, &0, provider.as_raw()) {
            let _ = SafeArrayDestroy(array);
            return error.code();
        }
    }
    output(out, array)
}

static SIMPLE_VT: IRawElementProviderSimple_Vtbl = IRawElementProviderSimple_Vtbl {
    base__: unknown::<SIMPLE>(),
    ProviderOptions: options,
    GetPatternProvider: pattern,
    GetPropertyValue: property,
    HostRawElementProvider: host,
};
static FRAGMENT_VT: IRawElementProviderFragment_Vtbl = IRawElementProviderFragment_Vtbl {
    base__: unknown::<FRAGMENT>(),
    Navigate: navigate,
    GetRuntimeId: runtime_id,
    BoundingRectangle: bounds,
    GetEmbeddedFragmentRoots: embedded,
    SetFocus: focus,
    FragmentRoot: fragment_root,
};
static ROOT_VT: IRawElementProviderFragmentRoot_Vtbl = IRawElementProviderFragmentRoot_Vtbl {
    base__: unknown::<ROOT>(),
    ElementProviderFromPoint: from_point,
    GetFocus: get_focus,
};
static SELECT_VT: ISelectionItemProvider_Vtbl = ISelectionItemProvider_Vtbl {
    base__: unknown::<SELECT_ITEM>(),
    Select: select,
    AddToSelection: add_selection,
    RemoveFromSelection: remove_selection,
    IsSelected: selected,
    SelectionContainer: selection_container,
};
static EXPAND_VT: IExpandCollapseProvider_Vtbl = IExpandCollapseProvider_Vtbl {
    base__: unknown::<EXPAND>(),
    Expand: expand,
    Collapse: collapse,
    ExpandCollapseState: expansion_state,
};
static VALUE_VT: IValueProvider_Vtbl = IValueProvider_Vtbl {
    base__: unknown::<VALUE>(),
    SetValue: set_value,
    Value: value,
    IsReadOnly: read_only,
};
static SELECTION_VT: ISelectionProvider_Vtbl = ISelectionProvider_Vtbl {
    base__: unknown::<SELECTION>(),
    GetSelection: get_selection,
    CanSelectMultiple: false_value,
    IsSelectionRequired: selection_required,
};

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Ole::{SafeArrayGetElement, SafeArrayGetUBound};

    fn row(id: &str, parent: Option<&str>, name: &str, selected: bool) -> AccessibleRow {
        AccessibleRow { id: id.into(), parent: parent.map(str::to_owned), name: name.into(), value: "PID 41; Private working set 1.0 GiB; Private commit 6.0 GiB; Change unavailable: Need five minutes of history".into(), depth: usize::from(parent.is_some()), has_children: parent.is_none(), expanded: parent.is_none(), selected, rect: RECT { left: 20, top: 30, right: 520, bottom: 62 } }
    }

    #[test]
    fn accessibility_client_can_read_hierarchy_selection_expansion_and_exact_values() {
        let state = Arc::new(Mutex::new(AccessibleTable {
            hwnd: null_mut(),
            sort: "Private working set descending".into(),
            focused: Some("instance:41".into()),
            rows: vec![
                row("name:app", None, "app.exe, 2 processes", false),
                row("instance:41", Some("name:app"), "app.exe, PID 41", true),
                row("name:other", None, "other.exe", false),
            ],
        }));
        let provider = install(null_mut(), state.clone());
        unsafe {
            let root: IRawElementProviderFragment = provider.root.cast().unwrap();
            let group = root.Navigate(NavigateDirection_FirstChild).unwrap();
            let child = group.Navigate(NavigateDirection_FirstChild).unwrap();
            let parent = child.Navigate(NavigateDirection_Parent).unwrap();
            let parent_simple: IRawElementProviderSimple = parent.cast().unwrap();
            let name = parent_simple.GetPropertyValue(UIA_NamePropertyId).unwrap();
            assert_eq!(
                name.Anonymous.Anonymous.Anonymous.bstrVal.to_string(),
                "app.exe, 2 processes"
            );
            let child_simple: IRawElementProviderSimple = child.cast().unwrap();
            let selection: ISelectionItemProvider = child_simple
                .GetPatternProvider(UIA_SelectionItemPatternId)
                .unwrap()
                .cast()
                .unwrap();
            assert!(selection.IsSelected().unwrap().as_bool());
            assert_eq!(
                selection.RemoveFromSelection().unwrap_err().code(),
                INVALID_OPERATION
            );
            let group_selection: ISelectionItemProvider = group.cast().unwrap();
            assert_eq!(
                group_selection.AddToSelection().unwrap_err().code(),
                INVALID_OPERATION
            );
            let values: IValueProvider = child_simple
                .GetPatternProvider(UIA_ValuePatternId)
                .unwrap()
                .cast()
                .unwrap();
            assert!(values.IsReadOnly().unwrap().as_bool());
            assert!(
                values
                    .Value()
                    .unwrap()
                    .to_string()
                    .contains("Private commit 6.0 GiB")
            );
            assert!(
                values
                    .Value()
                    .unwrap()
                    .to_string()
                    .contains("Need five minutes of history")
            );
            let expanded: IExpandCollapseProvider = group.cast().unwrap();
            assert_eq!(
                expanded.ExpandCollapseState().unwrap(),
                ExpandCollapseState_Expanded
            );
            let sort: IValueProvider = provider
                .root
                .GetPatternProvider(UIA_ValuePatternId)
                .unwrap()
                .cast()
                .unwrap();
            assert_eq!(
                sort.Value().unwrap().to_string(),
                "Private working set descending"
            );
            let selection: ISelectionProvider = provider.root.cast().unwrap();
            assert!(!selection.CanSelectMultiple().unwrap().as_bool());
            assert!(selection.IsSelectionRequired().unwrap().as_bool());
            let selected = selection.GetSelection().unwrap();
            let upper = SafeArrayGetUBound(selected, 1).unwrap();
            assert_eq!(upper, 0);
            SafeArrayDestroy(selected).unwrap();
            let runtime = child.GetRuntimeId().unwrap();
            let mut original = 0i32;
            SafeArrayGetElement(runtime, &1, (&mut original as *mut i32).cast()).unwrap();
            SafeArrayDestroy(runtime).unwrap();
            state.lock().unwrap().rows.swap(1, 2);
            let runtime = child.GetRuntimeId().unwrap();
            let mut reordered = 0i32;
            SafeArrayGetElement(runtime, &1, (&mut reordered as *mut i32).cast()).unwrap();
            assert_eq!(original, reordered, "identity survives live row reorder");
            SafeArrayDestroy(runtime).unwrap();
            state
                .lock()
                .unwrap()
                .rows
                .retain(|row| row.id != "instance:41");
            assert_eq!(values.Value().unwrap_err().code(), UNAVAILABLE);
        }
    }

    #[test]
    fn absent_neighbours_and_unsupported_patterns_write_null_at_com_boundary() {
        let state = Arc::new(Mutex::new(AccessibleTable {
            hwnd: null_mut(),
            sort: "Private working set descending".into(),
            focused: None,
            rows: Vec::new(),
        }));
        let provider = install(null_mut(), state);
        unsafe {
            let root: IRawElementProviderFragment = provider.root.cast().unwrap();
            let mut result = std::ptr::dangling_mut::<c_void>();
            let hr =
                (root.vtable().Navigate)(root.as_raw(), NavigateDirection_FirstChild, &mut result);
            assert_eq!(hr, S_OK);
            assert!(result.is_null());
            result = std::ptr::dangling_mut::<c_void>();
            let hr = (provider.root.vtable().GetPatternProvider)(
                provider.root.as_raw(),
                UIA_InvokePatternId,
                &mut result,
            );
            assert_eq!(hr, S_OK);
            assert!(result.is_null());
            let values: IValueProvider = provider.root.cast().unwrap();
            drop(provider);
            assert_eq!(
                values.Value().unwrap_err().code(),
                UNAVAILABLE,
                "retained COM interfaces fail safely after their window provider is released"
            );
        }
    }
}
