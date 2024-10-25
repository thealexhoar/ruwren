use std::io::Write;

use proc_macro2::Span;
use quote::{quote, quote_spanned, ToTokens};
use syn::{
    braced, parse::Parse, parse_macro_input, parse_quote, punctuated::Punctuated, spanned::Spanned,
    Data, DeriveInput, ImplItem, ImplItemFn, ReturnType, Token, Type, Visibility,
};

fn generate_wrapper_type_name(name: &syn::Ident) -> syn::Ident {
    syn::Ident::new(&format!("{name}Wrapper"), Span::call_site())
}

fn generate_class_type_name(name: &syn::Ident) -> syn::Ident {
    syn::Ident::new(&format!("{name}Class"), Span::call_site())
}

fn generate_instance_type_name(name: &syn::Ident) -> syn::Ident {
    syn::Ident::new(&format!("{name}Instance"), Span::call_site())
}

fn generate_class_type(tp: &syn::TypePath) -> syn::TypePath {
    let qself = tp.qself.clone();
    let mut path = tp.path.clone();
    let last_item = path
        .segments
        .last_mut()
        .unwrap_or_else(|| panic!("{:?} has no last component", tp));
    last_item.ident = generate_class_type_name(&last_item.ident);
    syn::TypePath { qself, path }
}

fn generate_instance_type(tp: &syn::TypePath) -> syn::TypePath {
    let qself = tp.qself.clone();
    let mut path = tp.path.clone();
    let last_item = path
        .segments
        .last_mut()
        .unwrap_or_else(|| panic!("{:?} has no last component", tp));
    last_item.ident = generate_instance_type_name(&last_item.ident);
    syn::TypePath { qself, path }
}

fn generate_class(
    name: &syn::Ident, fields: &syn::Fields, field_data: &[(&syn::Field, WrenObjectFieldDecl)],
) -> proc_macro2::TokenStream {
    let cname = generate_class_type_name(name);
    match fields {
        syn::Fields::Unit => {
            quote! {
                struct #cname;

                impl From<#name> for #cname {
                    #[inline]
                    fn from(source: #name) -> Self {
                        Self
                    }
                }
            }
        }
        syn::Fields::Named(_) => {
            let valid: Vec<_> = field_data
                .iter()
                .filter_map(|(f, decl)| if decl.static_member { Some(*f) } else { None })
                .collect();
            let extract: Vec<_> = valid
                .iter()
                .map(|f| {
                    let name = f.ident.as_ref().unwrap();
                    quote_spanned! {f.span()=>
                        #name: source.#name
                    }
                })
                .collect();
            let decls: Vec<_> = valid
                .into_iter()
                .map(|f| {
                    // We can unwrap, because fields are definitely named
                    let name = f.ident.as_ref().unwrap();
                    let ty = &f.ty;
                    quote_spanned! {f.span()=>
                        #name: #ty
                    }
                })
                .collect();
            quote! {
                struct #cname {
                    #(
                        #decls
                    ),*
                }

                impl From<#name> for #cname {
                    #[inline]
                    fn from(source: #name) -> Self {
                        Self {
                            #(
                                #extract
                            ),*
                        }
                    }
                }
            }
        }
        syn::Fields::Unnamed(_) => {
            let valid: Vec<_> = field_data
                .iter()
                .enumerate()
                .filter_map(|(i, (f, decl))| {
                    if decl.static_member {
                        Some((i, f))
                    } else {
                        None
                    }
                })
                .collect();
            if !valid.is_empty() {
                let extract: Vec<_> = valid
                    .iter()
                    .map(|(src_idx, f)| {
                        let idx = syn::Index::from(*src_idx);
                        quote_spanned! {f.span()=>
                            source.#idx
                        }
                    })
                    .collect();
                let decls: Vec<_> = valid
                    .into_iter()
                    .map(|(_, f)| {
                        let ty = &f.ty;
                        quote_spanned! {f.span()=>
                            #ty
                        }
                    })
                    .collect();
                quote! {
                    struct #cname (
                        #(
                            #decls
                        ),*
                    );

                    impl From<#name> for #cname {
                        #[inline]
                        fn from(source: #name) -> Self {
                            Self (
                                #(
                                    #extract
                                ),*
                            )
                        }
                    }
                }
            } else {
                quote! {
                    struct #cname;

                    impl From<#name> for #cname {
                        #[inline]
                        fn from(source: #name) -> Self {
                            Self
                        }
                    }
                }
            }
        }
    }
}

fn generate_instance(
    name: &syn::Ident, fields: &syn::Fields, field_data: &[(&syn::Field, WrenObjectFieldDecl)],
) -> proc_macro2::TokenStream {
    let iname = generate_instance_type_name(name);
    match fields {
        syn::Fields::Unit => {
            quote! {
                struct #iname;

                impl From<#name> for #iname {
                    #[inline]
                    fn from(source: #name) -> Self {
                        Self
                    }
                }
            }
        }
        syn::Fields::Named(_) => {
            let valid: Vec<_> = field_data
                .iter()
                .filter_map(|(f, decl)| if !decl.static_member { Some(*f) } else { None })
                .collect();
            let extract: Vec<_> = valid
                .iter()
                .map(|f| {
                    let name = f.ident.as_ref().unwrap();
                    quote_spanned! {f.span()=>
                        #name: source.#name
                    }
                })
                .collect();
            let decls: Vec<_> = valid
                .iter()
                .map(|f| {
                    // We can unwrap, because fields are definitely named
                    let name = f.ident.as_ref().unwrap();
                    let ty = &f.ty;
                    let vis = &f.vis;
                    quote_spanned! {f.span()=>
                        #vis #name: #ty
                    }
                })
                .collect();
            quote! {
                struct #iname {
                    #(
                        #decls
                    ),*
                }

                impl From<#name> for #iname {
                    #[inline]
                    fn from(source: #name) -> Self {
                        Self {
                            #(
                                #extract
                            ),*
                        }
                    }
                }
            }
        }
        syn::Fields::Unnamed(_) => {
            let valid: Vec<_> = field_data
                .iter()
                .enumerate()
                .filter_map(|(i, (f, decl))| {
                    if !decl.static_member {
                        Some((i, f))
                    } else {
                        None
                    }
                })
                .collect();
            if !valid.is_empty() {
                let extract: Vec<_> = valid
                    .iter()
                    .map(|(src_idx, f)| {
                        let idx = syn::Index::from(*src_idx);
                        quote_spanned! {f.span()=>
                            source.#idx
                        }
                    })
                    .collect();
                let decls: Vec<_> = valid
                    .into_iter()
                    .map(|(_, f)| {
                        let ty = &f.ty;
                        quote_spanned! {f.span()=>
                            #ty
                        }
                    })
                    .collect();
                quote! {
                    struct #iname (
                        #(
                            #decls
                        ),*
                    );

                    impl From<#name> for #iname {
                        #[inline]
                        fn from(source: #name) -> Self {
                            Self (
                                #(
                                    #extract
                                ),*
                            )
                        }
                    }
                }
            } else {
                quote! {
                    struct #iname;

                    impl From<#name> for #iname {
                        #[inline]
                        fn from(source: #name) -> Self {
                            Self
                        }
                    }
                }
            }
        }
    }
}

fn generate_wrapper(name: &syn::Ident) -> proc_macro2::TokenStream {
    let wname = generate_wrapper_type_name(name);
    let iname = generate_instance_type_name(name);
    let cname = generate_class_type_name(name);

    quote! {
        struct #wname<'a> {
            class: &'a mut #cname,
            instance: &'a mut #iname,
        }

        impl<'a> From<&#wname<'a>> for #name {
            #[inline]
            fn from(wrapper: &#wname<'a>) -> Self {
                (&*wrapper.class, &*wrapper.instance).into()
            }
        }

        impl<'a> From<(&'a mut #cname, &'a mut #iname)> for #wname<'a> {
            #[inline]
            fn from((class, instance): (&'a mut #cname, &'a mut #iname)) -> Self {
                Self { class, instance }
            }
        }

        impl<'a> std::ops::Deref for #wname<'a> {
            type Target = #iname;
            #[inline]
            fn deref(&self) -> &#iname {
                &self.instance
            }
        }

        impl<'a> std::ops::DerefMut for #wname<'a> {
            #[inline]
            fn deref_mut(&mut self) -> &mut #iname {
                &mut self.instance
            }
        }
    }
}

fn generate_enhancements(
    name: &syn::Ident, fields: &syn::Fields, field_data: &[(&syn::Field, WrenObjectFieldDecl)],
) -> proc_macro2::TokenStream {
    let class_name = generate_class_type_name(name);
    let instance_name = generate_instance_type_name(name);

    let from_impl = match fields {
        syn::Fields::Unit => {
            quote! {
                Self
            }
        }
        syn::Fields::Named(_) => {
            let extract: Vec<_> = field_data
                .iter()
                .map(|(f, dat)| {
                    // We can unwrap, because fields are definitely named
                    let name = f.ident.as_ref().unwrap();
                    if dat.static_member {
                        quote_spanned! {f.span()=>
                            #name: class.#name.clone()
                        }
                    } else {
                        quote_spanned! {f.span()=>
                            #name: inst.#name.clone()
                        }
                    }
                })
                .collect();
            quote! {
                Self {
                    #(
                        #extract
                    ),*
                }
            }
        }
        syn::Fields::Unnamed(_) => {
            if !field_data.is_empty() {
                let extract: Vec<_> = field_data
                    .iter()
                    .scan((0, 0), |(ci, ii), (f, dat)| {
                        // We can unwrap, because fields are definitely named
                        if dat.static_member {
                            let idx = syn::Index::from(*ci);
                            *ci += 1;
                            Some(quote! {
                                class.#idx.clone()
                            })
                        } else {
                            let idx = syn::Index::from(*ii);
                            *ii += 1;
                            Some(quote_spanned! {f.span()=>
                                inst.#idx.clone()
                            })
                        }
                    })
                    .collect();
                quote! {
                    Self (
                        #(
                            #extract
                        ),*
                    )
                }
            } else {
                quote! {
                    Self
                }
            }
        }
    };

    quote! {
        impl<'a> From<(&'a #class_name, &'a #instance_name)> for #name {
            #[allow(clippy::clone_on_copy)]
            #[inline]
            fn from((class, inst): (&'a #class_name, &'a #instance_name)) -> Self {
                #from_impl
            }
        }

        impl TryFrom<Option<#name>> for #name {
            type Error = ();

            fn try_from(value: Option<#name>) -> Result<Self, Self::Error> {
                value.ok_or(())
            }
        }
    }
}

#[derive(deluxe::ExtractAttributes)]
#[deluxe(attributes(wren))]
struct WrenObjectFieldDecl {
    #[deluxe(default)]
    static_member: bool,
}

#[proc_macro_derive(WrenObject, attributes(wren))]
pub fn wren_object_derive(stream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(stream as DeriveInput);

    let mut struct_impl = match input.data {
        Data::Struct(s) => s,
        _ => {
            return quote! {
                compile_error!("only structs are supported")
            }
            .into()
        }
    };

    let errors = deluxe::Errors::new();

    let field_decls: Option<Vec<WrenObjectFieldDecl>> = struct_impl
        .fields
        .iter_mut()
        .map(|f| match deluxe::extract_attributes(f) {
            Ok(fd) => Some(fd),
            Err(e) => {
                errors.push_syn(e);
                None
            }
        })
        .collect();

    let field_decls = if let Some(field_decls) = field_decls {
        struct_impl.fields.iter().zip(field_decls).collect()
    } else {
        errors.push_call_site("A field decl extractor failed.");
        vec![]
    };

    let class_type = generate_class(&input.ident, &struct_impl.fields, &field_decls);
    let instance_type = generate_instance(&input.ident, &struct_impl.fields, &field_decls);
    let enhancements = generate_enhancements(&input.ident, &struct_impl.fields, &field_decls);
    let wrapper_type = generate_wrapper(&input.ident);
    let vis = &input.vis;

    let expanded = quote! {
        #errors
        #enhancements
        #vis #class_type
        #vis #instance_type
        #vis #wrapper_type
    };

    println!("--- wren_object_derive -----------------------------");
    writeln!(std::io::stdout(), "{}", expanded);
    proc_macro::TokenStream::from(expanded)
}

#[derive(Clone, Default, deluxe::ExtractAttributes)]
#[deluxe(default, attributes(wren_impl))]
struct WrenImplFnAttrs {
    // [0, 1] required (if 0, will attempt to use Default on Foo to generate FooClass)
    allocator: bool,
    // [0, 1] required (if 0, will attempt to use Default on Foo to generate FooInstance)
    constructor: bool,

    instance: bool,
    getter: bool,
    setter: bool,

    ignore: bool, // Alex: I added this

    object: Vec<syn::Ident>,
}

struct WrenImplValidFn {
    receiver_ty: syn::Type,
    is_static: bool,
    is_setter: bool,
    is_getter: bool,
    source_name: Option<syn::Ident>,
    normal_params: Vec<(usize, syn::PatType)>,
    object_params: Vec<(usize, syn::PatType)>,
    func: ImplItemFn,
}

struct FindInnerType {
    discovered_tp: Option<syn::TypePath>,
}

impl<'a> syn::visit::Visit<'a> for FindInnerType {
    fn visit_type_path(&mut self, tp: &'a syn::TypePath) {
        if let Some(p) = tp.path.segments.last() {
            match &p.arguments {
                syn::PathArguments::AngleBracketed(args) => {
                    self.visit_angle_bracketed_generic_arguments(args);
                }
                syn::PathArguments::None => {
                    self.discovered_tp = Some(tp.clone());
                }
                _ => {}
            }
        }
    }

    fn visit_angle_bracketed_generic_arguments(
        &mut self, args: &'a syn::AngleBracketedGenericArguments,
    ) {
        if let Some(t) = args.args.iter().find_map(|a| match a {
            syn::GenericArgument::Type(t) => Some(t),
            _ => None,
        }) {
            self.visit_type(t)
        }
    }
}

impl WrenImplValidFn {
    fn arity(&self) -> usize {
        self.normal_params.len() + self.object_params.len()
    }

    fn source_name(&self) -> &syn::Ident {
        self.source_name.as_ref().unwrap_or(&self.func.sig.ident)
    }

    fn base_name(&self) -> &syn::Ident {
        &self.func.sig.ident
    }

    /// Generate the body for [`Self::gen_vm_fn()`] and [`Self::gen_vm_fn_constructor()`]
    fn gen_vm_fn_body(
        &self, source_name: &syn::Ident, constructor_mode: bool,
    ) -> proc_macro2::TokenStream {
        let (normal_extract, normal_arg): (Vec<_>, Vec<_>) = self
            .normal_params
            .iter()
            .map(|(idx, ty)| {
                let slot_idx = idx + 1;
                let arg_name = syn::Ident::new(&format!("arg{}", idx), Span::call_site());
                let arg_slot_name = syn::Ident::new(&format!("arg{}_calc", idx), Span::call_site());
                let ty = &*ty.ty;
                let arity = self.arity();
                let call = if *idx == 0 {
                    quote! {
                        new::<#ty>(#slot_idx, #arity)
                    }
                } else {
                    let prev_arg_slot_name =
                        syn::Ident::new(&format!("arg{}_calc", idx - 1), Span::call_site());

                    quote! {
                        next::<#ty>(#slot_idx, &#prev_arg_slot_name)
                    }
                };
                let failure = if constructor_mode {
                    quote! {
                        return Err(format!("failed to get value of type {} for slot {}", std::any::type_name::<#ty>(), #slot_idx));
                    }
                } else {
                    quote! {
                        ruwren::foreign_v2::WrenTo::to_vm(format!("failed to get value of type {} for slot {}", std::any::type_name::<#ty>(), #slot_idx), vm, 0, 1);
                        vm.abort_fiber(0);
                        return
                    }
                };
                (
                    (idx, quote! {
                        let #arg_slot_name = ruwren::foreign_v2::InputSlot::#call
                    }),
                    quote! {
                        let Some(#arg_name): Option<#ty> = ruwren::foreign_v2::get_slot_value(vm, &#arg_slot_name, #arity) else {
                            #failure
                        }
                    },
                )
            })
            .unzip();
        let (object_extract, object_arg): (Vec<_>, Vec<_>) = self
        .object_params
        .iter()
        .map(|(idx, ty)| {
            use syn::visit::Visit;

            let slot_idx = idx + 1;
            let arg_name = syn::Ident::new(&format!("arg{}", idx), Span::call_site());
            let arg_slot_name = syn::Ident::new(&format!("arg{}_calc", idx), Span::call_site());
            let ty = &*ty.ty;
            let mut fit = FindInnerType { discovered_tp: None };
            fit.visit_type(ty);
            let source_type = fit.discovered_tp.take().map(|tp| {
                let inst_ty = generate_instance_type(&tp);
                quote_spanned! {tp.span()=>
                    #inst_ty
                }
            }).unwrap_or(quote! {
                compile_error!("invalid object type")
            });
            let arity = self.arity();
            let call = if *idx == 0 {
                quote! {
                    object_new(#slot_idx, #arity)
                }
            } else {
                let prev_arg_slot_name =
                    syn::Ident::new(&format!("arg{}_calc", idx - 1), Span::call_site());

                quote! {
                    object_next(#slot_idx, &#prev_arg_slot_name)
                }
            };
            let receiver = if self.is_static {
                quote! {self}
            } else {
                quote! {self.class}
            };
            let failure = if constructor_mode {
                quote! {
                    return Err(format!("failed to get value of type {} for slot {}", std::any::type_name::<#ty>(), #slot_idx));
                }
            } else {
                quote! {
                    ruwren::foreign_v2::WrenTo::to_vm(format!("failed to get value of type {} for slot {}", std::any::type_name::<#ty>(), #slot_idx), vm, 0, 1);
                    vm.abort_fiber(0);
                    return
                }
            };
            (
                (idx, quote! {
                    let #arg_slot_name = ruwren::foreign_v2::InputSlot::#call
                }),
                quote! {
                    let Some(#arg_name): Option<#ty> = ruwren::foreign_v2::get_slot_object::<#source_type, _>(vm, &#arg_slot_name, #arity, #receiver) else {
                        #failure
                    }
                },
            )
        })
        .unzip();

        let call = {
            let mut call_args: Vec<_> = self
                .object_params
                .iter()
                .map(|(i, d)| (i, d, true))
                .chain(self.normal_params.iter().map(|(i, d)| (i, d, false)))
                .collect();
            call_args.sort_by(|(a, _, _), (b, _, _)| a.cmp(b));
            let input_args = call_args.into_iter().map(|(idx, dat, is_obj)| {
                let arg_name = syn::Ident::new(&format!("arg{}", idx), Span::call_site());
                let slot_idx = idx + 1;
                let ty = &dat.ty;
                if is_obj {
                    quote! {
                        match #arg_name.try_into() {
                            Ok(v) => v,
                            Err(_) => panic!(
                                "slot {} cannot be type {}",
                                #slot_idx,
                                std::any::type_name::<#ty>()
                            ),
                        }
                    }
                } else {
                    quote! {
                        #arg_name
                    }
                }
            });

            let class_name = generate_class_type_name(source_name);
            let wrapper_name = generate_wrapper_type_name(source_name);
            let name = &self.base_name();

            if self.is_static {
                quote! {
                    #class_name::#name(self, #(#input_args),*)
                }
            } else {
                quote! {
                    #wrapper_name::#name(self, #(#input_args),*)
                }
            }
        };

        let mut extractors: Vec<_> = normal_extract.into_iter().chain(object_extract).collect();
        extractors.sort_by(|(a, _), (b, _)| a.cmp(b));
        let extractors: Vec<_> = extractors.into_iter().map(|(_, e)| e).collect();

        quote! {
            #(
                #extractors
            );*;

            #(
                #normal_arg
            );*;
            #(
                #object_arg
            );*;
            let ret = #call;
        }
    }

    /// Generate a wrapper around this function that takes a receiver
    /// and the vm as arguments and returns an instance
    fn gen_vm_fn_constructor(&self, source_name: &syn::Ident) -> proc_macro2::TokenStream {
        let wrapper_fn_name =
            syn::Ident::new(&format!("vm_{}", self.base_name()), Span::call_site());
        let instance_name = generate_instance_type_name(source_name);
        let vis = &self.func.vis;
        let body = self.gen_vm_fn_body(source_name, true);
        quote! {
            #[inline]
            #vis fn #wrapper_fn_name(&mut self, vm: &ruwren::VM) -> Result<#instance_name, String> {
                #body
                ret
            }
        }
    }

    /// Generate a wrapper around this function that takes a receiver
    /// and the vm as arguments
    fn gen_vm_fn(&self, source_name: &syn::Ident) -> proc_macro2::TokenStream {
        let wrapper_fn_name =
            syn::Ident::new(&format!("vm_{}", self.base_name()), Span::call_site());
        let body = self.gen_vm_fn_body(source_name, false);
        quote_spanned! {self.func.span()=>
            #[inline(always)]
            fn #wrapper_fn_name(&mut self, vm: &ruwren::VM) {
                #body
                ruwren::foreign_v2::WrenTo::to_vm(ret, vm, 0, 1);
            }
        }
    }

    /// Generate a wrapper around this function that takes a
    /// *mut ruwren::wren_sys::WrenVM as an argument.
    ///
    /// Calls [`Self::gen_vm_fn()`] internally to generate the function that
    /// this wrapper calls.
    ///
    /// This wrapper function is FFI-safe. (or at least, should be)
    fn gen_native_vm_fn(&self, source_name: &syn::Ident) -> proc_macro2::TokenStream {
        let wrapper_fn = self.gen_vm_fn(source_name);
        let wrapper_fn_name =
            syn::Ident::new(&format!("vm_{}", self.base_name()), Span::call_site());
        let native_name = syn::Ident::new(
            &format!("native_vm_{}", self.base_name()),
            Span::call_site(),
        );
        let instance_name = generate_instance_type_name(source_name);
        let class_name = generate_class_type_name(source_name);
        let wrapper_name = generate_wrapper_type_name(source_name);
        let vis = &self.func.vis;
        let native_wrapper = if self.is_static {
            quote! {
                #vis unsafe extern "C" fn #native_name(vm: *mut ruwren::wren_sys::WrenVM) {
                    use std::panic::{set_hook, take_hook, AssertUnwindSafe};

                    let conf = std::ptr::read_unaligned(
                        ruwren::wren_sys::wrenGetUserData(vm) as *mut ruwren::UserData
                    );
                    let ovm = vm;
                    let vm = std::rc::Weak::upgrade(&conf.vm)
                        .unwrap_or_else(|| panic!("Failed to access VM at {:p}", &conf.vm));
                    set_hook(Box::new(|_| {}));
                    let vm_borrow = AssertUnwindSafe(vm.borrow());
                    {
                        use ruwren::foreign_v2::V2Class;
                        vm_borrow.use_class_mut::<#instance_name, _, _>(|vm, cls| {
                            let class =
                                cls.unwrap_or_else(|| panic!("Failed to resolve class for {}", #class_name::name()));
                            #class_name::#wrapper_fn_name(class, vm)
                        })
                    };
                    drop(take_hook());
                    std::ptr::write_unaligned(
                        ruwren::wren_sys::wrenGetUserData(ovm) as *mut ruwren::UserData,
                        conf,
                    );
                }
            }
        } else {
            quote! {
                #vis unsafe extern "C" fn #native_name(vm: *mut ruwren::wren_sys::WrenVM) {
                    use std::panic::{set_hook, take_hook, AssertUnwindSafe};

                    let conf = std::ptr::read_unaligned(
                        ruwren::wren_sys::wrenGetUserData(vm) as *mut ruwren::UserData
                    );
                    let ovm = vm;
                    let vm = std::rc::Weak::upgrade(&conf.vm)
                        .unwrap_or_else(|| panic!("Failed to access VM at {:p}", &conf.vm));
                    set_hook(Box::new(|_pi| {}));
                    let vm_borrow = AssertUnwindSafe(vm.borrow());
                    {
                        use ruwren::foreign_v2::V2Class;
                        vm_borrow.ensure_slots(1);
                        let inst = vm_borrow
                            .get_slot_foreign_mut::<#instance_name>(0)
                            .unwrap_or_else(|| panic!(
                                "Tried to call {0} of {1} on non-{1} type",
                                stringify!($inf),
                                std::any::type_name::<#instance_name>()
                            ));
                        vm_borrow.use_class_mut::<#instance_name, _, _>(|vm, cls| {
                            let class =
                                cls.unwrap_or_else(|| panic!("Failed to resolve class for {}", #class_name::name()));
                            let mut wrapper: #wrapper_name = (class, inst).into();
                            wrapper.#wrapper_fn_name(vm)
                        })
                    };
                    drop(take_hook());
                    std::ptr::write_unaligned(
                        ruwren::wren_sys::wrenGetUserData(ovm) as *mut ruwren::UserData,
                        conf,
                    );
                }
            }
        };

        quote! {
            #wrapper_fn
            #native_wrapper
        }
    }
}

#[derive(Clone)]
struct WrenImplFn {
    func: ImplItemFn,
    attrs: WrenImplFnAttrs,
}

impl TryFrom<(&syn::Ident, WrenImplFn)> for WrenImplValidFn {
    type Error = Vec<String>;

    fn try_from((src, value): (&syn::Ident, WrenImplFn)) -> Result<Self, Self::Error> {
        let (receiver_ty, args, has_self): (syn::Type, _, _) =
            if value.func.sig.receiver().is_some() {
                let class_type = generate_class_type_name(src);
                let wrapper_type = generate_wrapper_type_name(src);
                (
                    if value.attrs.instance {
                        parse_quote!( #wrapper_type<'a> )
                    } else {
                        parse_quote!( #class_type )
                    },
                    value.func.sig.inputs.clone(),
                    true,
                )
            } else {
                let Some(arg) = value
                    .func
                    .sig
                    .inputs
                    .iter()
                    .nth(0)
                    .and_then(|fna| match fna {
                        syn::FnArg::Typed(aty) => Some(aty),
                        _ => None,
                    })
                else {
                    return Err(vec![format!(
                        "method {} must have a receiver",
                        value.func.sig.ident
                    )]);
                };
                let inputs = value.func.sig.inputs.clone().into_iter().skip(1).collect();

                ((*arg.ty).clone(), inputs, false)
            };

        let object_param_pairs: Vec<_> = value
            .attrs
            .object
            .iter()
            .map(|name| {
                (
                    name,
                    args.iter().find(|i| match i {
                        syn::FnArg::Receiver(_) => false,
                        syn::FnArg::Typed(ty) => match &*ty.pat {
                            syn::Pat::Ident(i) => i.ident == *name,
                            _ => false,
                        },
                    }),
                )
            })
            .collect();

        let (object_params, normal_params): (Vec<_>, Vec<_>) = args
            .iter()
            .filter_map(|fna| match fna {
                syn::FnArg::Receiver(_) => None,
                syn::FnArg::Typed(ty) => Some(ty),
            })
            .cloned()
            .enumerate()
            .partition(|(_, arg)| match &*arg.pat {
                syn::Pat::Ident(i) => value.attrs.object.contains(&i.ident),
                _ => false,
            });

        let mut errors: Vec<_> = object_param_pairs
            .into_iter()
            .filter_map(|(name, arg)| {
                if arg.is_none() {
                    Some(format!("Could not find top-level object argument {}", name))
                } else {
                    None
                }
            })
            .collect();

        let mut given_name = None;

        let is_setter = if value.attrs.setter {
            let output = &value.func.sig.output;
            let count = if has_self { 2 } else { 1 };
            if args.len() == count
                && (*output == syn::ReturnType::Default || *output == parse_quote! { -> ()})
            {
                given_name = Some(syn::Ident::new(
                    &format!("setter_{}", value.func.sig.ident),
                    Span::call_site(),
                ));
                true
            } else {
                errors.push(format!(
                    "setter {} must take 1 non-receiver argument (takes {} arguments), and return () (returns {})",
                    value.func.sig.ident,
                    args.len(),
                    match output {
                        syn::ReturnType::Default => parse_quote!{()},
                        syn::ReturnType::Type(_, ty) => ty.into_token_stream(),
                    }
                ));
                false
            }
        } else {
            false
        };

        let is_getter = if value.attrs.getter {
            let count = if has_self { 1 } else { 0 };
            if args.len() == count {
                given_name = Some(syn::Ident::new(
                    &format!("getter_{}", value.func.sig.ident),
                    Span::call_site(),
                ));
                true
            } else {
                errors.push(format!(
                    "getter {} must take no non-receiver arguments (takes {} arguments)",
                    value.func.sig.ident,
                    args.len(),
                ));
                false
            }
        } else {
            false
        };

        if !errors.is_empty() {
            Err(errors)
        } else {
            let mut func = value.func;
            let source_name = if let Some(given_name) = given_name {
                let source_name = func.sig.ident.clone();
                func.sig.ident = given_name;
                Some(source_name)
            } else {
                None
            };
            Ok(Self {
                receiver_ty,
                is_getter,
                is_setter,
                source_name,
                is_static: !value.attrs.instance,
                func,
                normal_params,
                object_params,
            })
        }
    }
}

impl WrenImplFn {
    fn validate_allocator(&mut self, ty: &syn::Ident) -> Result<(), Vec<String>> {
        let class_ty = generate_class_type_name(ty);

        let mut errors = vec![];

        if !self.func.sig.inputs.is_empty() {
            errors.push("allocators cannot take any parameters".to_string());
        }

        match self.func.sig.output {
            ReturnType::Default => {
                self.func.sig.output = parse_quote! { -> #class_ty };
            }
            ReturnType::Type(_, ref ty) => match ty.as_ref() {
                Type::Path(p) => {
                    let last = p.path.segments.last();
                    if last.is_none() || last.is_some_and(|name| name.ident != class_ty) {
                        errors.push(format!(
                            "allocators must return {}, but allocator returned {}",
                            class_ty.into_token_stream(),
                            p.into_token_stream()
                        ))
                    }
                }
                ty => match ty {
                    Type::Infer(_) => {
                        self.func.sig.output = parse_quote! { -> #class_ty };
                    }
                    ty => errors.push(format!(
                        "allocators must return {}, but allocator returned {}",
                        class_ty.into_token_stream(),
                        ty.into_token_stream()
                    )),
                },
            },
        }

        if !errors.is_empty() {
            Err(errors)
        } else {
            Ok(())
        }
    }
}

impl Parse for WrenImplFn {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let item: ImplItem = input.parse()?;
        match item {
            ImplItem::Fn(mut func) => {
                let attrs = deluxe::extract_attributes(&mut func)?;
                Ok(Self { func, attrs })
            }
            _ => unimplemented!(),
        }
    }
}

struct WrenObjectImpl {
    ty: syn::Ident,
    items: Vec<WrenImplFn>,
}

struct WrenObjectValidImpl {
    ty: syn::Ident,
    allocator: Option<WrenImplFn>,
    constructor: Option<WrenImplValidFn>,
    others: Vec<WrenImplValidFn>,
}

impl WrenObjectImpl {
    fn validate(self) -> Result<WrenObjectValidImpl, Vec<String>> {
        let allocators: Vec<_> = self.items.iter().filter(|fi| fi.attrs.allocator).collect();
        let constructors: Vec<_> = self
            .items
            .iter()
            .filter(|fi| fi.attrs.constructor)
            .collect();
        let mut errors = vec![];

        let mut allocator = if allocators.len() <= 1 {
            allocators.first().cloned().cloned()
        } else {
            return Err(vec![format!(
                "Expected 0 or 1 allocators, found {}",
                allocators.len()
            )]);
        };

        if let Some(ref mut allocator) = allocator {
            if let Err(errs) = allocator.validate_allocator(&self.ty) {
                errors.extend(errs)
            }
        }

        let constructor = if constructors.len() <= 1 {
            constructors.first().cloned().cloned()
        } else {
            return Err(vec![format!(
                "Expected 0 or 1 constructors, found {}",
                constructors.len()
            )]);
        };

        let constructor = if let Some(constructor) = constructor {
            let instance_name = generate_instance_type_name(&self.ty);
            let class_name = generate_class_type_name(&self.ty);
            match TryInto::<WrenImplValidFn>::try_into((&self.ty, constructor)) {
                Ok(mut constructor) => {
                    match constructor.func.sig.output {
                        ReturnType::Default => {
                            constructor.func.sig.output =
                                parse_quote! { -> Result<#instance_name, String> };
                        }
                        ReturnType::Type(_, ref ty) => {
                            if let Type::Infer(_) = **ty {
                                constructor.func.sig.output =
                                    parse_quote! { -> Result<#instance_name, String> };
                            }
                        }
                    }
                    if constructor.func.sig.output
                        == parse_quote! {-> Result<#instance_name, String>}
                    {
                        if match constructor.receiver_ty {
                            Type::Reference(ref tr) => tr.elem == parse_quote! { #class_name },
                            Type::Path(ref tp) => tp.path == parse_quote! { #class_name },
                            _ => false,
                        } {
                            Some(constructor)
                        } else {
                            errors.push(format!(
                                "A constructor must receive &mut {0} (or &{0}), but it receives {1}",
                                class_name.into_token_stream(),
                                constructor.receiver_ty.into_token_stream(),
                            ));
                            None
                        }
                    } else {
                        errors.push(format!(
                            "A constructor must return {}, but it returns {}",
                            quote! { Result<#instance_name, String> },
                            constructor.func.sig.output.into_token_stream(),
                        ));
                        None
                    }
                }
                Err(errs) => {
                    errors.extend(errs);
                    None
                }
            }
        } else {
            None
        };

        let others: Vec<_> = self
            .items
            .iter()
            .filter(|fi| !fi.attrs.ignore && !fi.attrs.constructor && !fi.attrs.allocator)
            .cloned()
            .filter_map(|func| -> Option<WrenImplValidFn> {
                match (&self.ty, func).try_into() {
                    Ok(vfunc) => Some(vfunc),
                    Err(errs) => {
                        errors.extend(errs);
                        None
                    }
                }
            })
            .collect();

        if !errors.is_empty() {
            Err(errors)
        } else {
            Ok(WrenObjectValidImpl {
                ty: self.ty,
                allocator,
                constructor,
                others,
            })
        }
    }
}

impl Parse for WrenObjectImpl {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse::<Token![impl]>()?;
        let ty = input.parse()?;
        let content;
        braced!(content in input);
        let mut items = vec![];
        while !content.is_empty() {
            items.push(content.parse()?);
        }
        Ok(Self { ty, items })
    }
}

#[proc_macro_attribute]
pub fn wren_impl(
    _attr: proc_macro::TokenStream, item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let wren_object_impl = parse_macro_input!(item as WrenObjectImpl);

    let errors = deluxe::Errors::new();

    let wren_object_impl = match wren_object_impl.validate() {
        Ok(valid) => valid,
        Err(errs) => {
            for err in errs {
                errors.push_call_site(err)
            }
            return quote! {
                #errors
            }
            .into();
        }
    };

    let source_ty = &wren_object_impl.ty;
    let instance_ty = generate_instance_type_name(source_ty);
    let class_ty = generate_class_type_name(source_ty);
    let wrapper_ty = generate_wrapper_type_name(source_ty);

    let allocator_fn = match &wren_object_impl.allocator {
        Some(alloc) => {
            let func = &alloc.func;
            quote_spanned! {func.span()=>
                #func
            }
        }
        None => quote! {
            #[inline]
            fn ___default_alloc() -> #class_ty {
                use std::default::Default;
                #source_ty::default().into()
            }
        },
    };

    let constructor_fn = match &wren_object_impl.constructor {
        Some(constructor) => {
            let func = &constructor.func;
            let wrapper_func = constructor.gen_vm_fn_constructor(source_ty);
            quote_spanned! {func.span()=>
                #func
                #wrapper_func
            }
        }
        None => quote! {
            #[inline]
            fn ___default_constructor(&self) -> Result<#instance_ty, String> {
                use std::default::Default;
                Ok(#source_ty::default().into())
            }
        },
    };

    let allocator_call = match &wren_object_impl.allocator {
        Some(alloc) => {
            let name = &alloc.func.sig.ident;
            quote! {
                #class_ty::#name()
            }
        }
        None => {
            quote! {
                #class_ty::___default_alloc()
            }
        }
    };

    let constructor_call = match &wren_object_impl.constructor {
        Some(constructor) => {
            let name = &constructor.func.sig.ident;
            let wrapper_name = syn::Ident::new(&format!("vm_{}", name), Span::call_site());
            quote! {
                #class_ty::#wrapper_name(class, vm)
            }
        }
        None => {
            quote! {
                #class_ty::___default_constructor(class)
            }
        }
    };
    let function_decls = wren_object_impl.others.iter().map(|func| {
        let name = func.source_name();
        let wrapper_name = syn::Ident::new(
            &format!("native_vm_{}", func.base_name()),
            Span::call_site(),
        );
        let is_static = func.is_static;
        let arity = func.arity();
        let receiver_ty = if func.is_static {
            &class_ty
        } else {
            &wrapper_ty
        };
        let sig = if func.is_getter {
            quote! { ruwren::FunctionSignature::new_getter(stringify!(#name)) }
        } else if func.is_setter {
            quote! { ruwren::FunctionSignature::new_setter(stringify!(#name)) }
        } else {
            quote! { ruwren::FunctionSignature::new_function(stringify!(#name), #arity) }
        };
        quote! {
            ruwren::MethodPointer {
                is_static: #is_static,
                signature: #sig,
                pointer: #receiver_ty::#wrapper_name,
            }
        }
    });

    let static_fns = wren_object_impl
        .others
        .iter()
        .filter(|of| of.is_static)
        .map(|func| {
            let wrapper_func = func.gen_native_vm_fn(source_ty);
            let func = &func.func;
            quote_spanned! {func.span()=>
                #func
                #wrapper_func
            }
        });

    let instance_fns = wren_object_impl
        .others
        .iter()
        .filter(|of| !of.is_static)
        .map(|func| {
            let wrapper_func = func.gen_native_vm_fn(source_ty);
            let func = &func.func;
            quote_spanned! {func.span()=>
                #func
                #wrapper_func
            }
        });

    let expanded = quote! {
        #errors
        impl #class_ty {
            #allocator_fn
            #constructor_fn
            #(
                #static_fns
            )*
        }

        impl<'a> #wrapper_ty<'a> {
            #(
                #instance_fns
            )*
        }

        impl ruwren::foreign_v2::Slottable<#source_ty> for #instance_ty {
            type Context = #class_ty;
            #[inline]
            fn scratch_size() -> usize
            where
                Self: Sized,
            {
                0
            }

            #[inline]
            fn get(
                ctx: &mut Self::Context, vm: &ruwren::VM, slot: ruwren::SlotId,
                _scratch_start: ruwren::SlotId,
            ) -> Option<#source_ty> {
                let inst = vm.get_slot_foreign::<Self>(slot)?;
                Some((&*ctx, inst).into())
            }
        }

        impl ruwren::ClassObject for #instance_ty {
            fn initialize_pointer() -> extern "C" fn(*mut ruwren::wren_sys::WrenVM)
            where
                Self: Sized,
            {
                extern "C" fn _constructor(vm: *mut ruwren::wren_sys::WrenVM) {
                    use ruwren::foreign_v2::ForeignItem;
                    use std::panic::{set_hook, take_hook, AssertUnwindSafe};
                    use ruwren::handle_panic as catch_unwind;
                    unsafe {
                        let ud = ruwren::wren_sys::wrenGetUserData(vm);
                        let conf = std::ptr::read_unaligned(ud as *mut ruwren::UserData);
                        let ovm = vm;
                        let vm = std::rc::Weak::upgrade(&conf.vm)
                            .unwrap_or_else(|| panic!("Failed to access VM at {:p}", &conf.vm));
                        // Allocate a new object, and move it onto the heap
                        set_hook(Box::new(|_pi| {}));
                        let vm_borrow = AssertUnwindSafe(vm.borrow());
                        match #instance_ty::create(&*vm_borrow)
                        {
                            Ok(object) => {
                                let wptr = ruwren::wren_sys::wrenSetSlotNewForeign(
                                    vm.borrow().vm,
                                    0,
                                    0,
                                    std::mem::size_of::<ruwren::ForeignObject<#instance_ty>>()
                                );

                                std::ptr::write(
                                    wptr as *mut _,
                                    ruwren::ForeignObject {
                                        object: Box::into_raw(Box::new(object)),
                                        type_id: std::any::TypeId::of::<#instance_ty>(),
                                    },
                                );
                            },
                            Err(err_string) => {
                                vm_borrow.set_slot_string(0, err_string);
                                vm_borrow.abort_fiber(0);
                            }
                        };
                        drop(take_hook());
                        std::ptr::write_unaligned(
                            ud as *mut ruwren::UserData,
                            conf
                        );
                        ruwren::wren_sys::wrenSetUserData(ovm, ud);
                    }
                }
                _constructor
            }

            fn finalize_pointer() -> extern "C" fn(*mut std::ffi::c_void)
            where
                Self: Sized,
            {
                extern "C" fn _destructor(data: *mut std::ffi::c_void) {
                    unsafe {
                        let mut fo: ruwren::ForeignObject<#instance_ty> =
                            std::ptr::read_unaligned(data as *mut _);
                        if !fo.object.is_null() {
                            _ = Box::from_raw(fo.object);
                        }
                        fo.object = std::ptr::null_mut();
                        std::ptr::write_unaligned(data as *mut _, fo);
                    }
                }

                _destructor
            }

            fn generate_pointers() -> ruwren::ClassObjectPointers
            where
                Self: Sized,
            {
                ruwren::ClassObjectPointers {
                    function_pointers: vec![
                        #(
                            #function_decls
                        ),*
                    ]
                }
            }
        }

        impl ruwren::foreign_v2::V2Class for #class_ty {
            #[inline]
            fn name() -> &'static str {
                stringify!(#source_ty)
            }

            #[inline]
            fn allocate() -> Self {
                #allocator_call
            }
        }

        impl ruwren::foreign_v2::ForeignItem for #instance_ty {
            type Class = #class_ty;
            type Source = #source_ty;

            #[inline]
            fn construct(class: &mut Self::Class, vm: &ruwren::VM) -> Result<Self, String> {
                #constructor_call
            }
        }
    };
    println!("--- wren_impl -----------------------------");
    writeln!(std::io::stdout(), "{}", expanded);
    proc_macro::TokenStream::from(expanded)
}

struct WrenModuleItem {
    ty: syn::TypePath,
}

impl Parse for WrenModuleItem {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse::<Token![pub]>()?;
        let ty = input.parse()?;
        Ok(Self { ty })
    }
}

struct WrenModuleDecl {
    vis: syn::Visibility,
    name: syn::Ident,
    items: Punctuated<WrenModuleItem, Token![;]>,
}

impl Parse for WrenModuleDecl {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let vis = input.parse()?;
        input.parse::<Token![mod]>()?;
        let name: syn::Ident = input.parse()?;
        let content;
        braced!(content in input);
        let items = content.parse_terminated(WrenModuleItem::parse, Token![;])?;
        Ok(Self { vis, name, items })
    }
}

#[proc_macro]
pub fn wren_module(stream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let wren_module_decl = parse_macro_input!(stream as WrenModuleDecl);

    let vis = wren_module_decl.vis;
    let name = wren_module_decl.name;
    let (decls, to_impls): (Vec<_>, Vec<_>) = wren_module_decl
        .items
        .iter()
        .map(|mi| {
            let source_ty = &mi.ty;
            let class_ty = generate_class_type(source_ty);
            let instance_ty = generate_instance_type(source_ty);
            (
                quote_spanned! {mi.ty.span()=>
                    module.class::<#instance_ty, _>(#class_ty::name());
                },
                quote! {
                    impl ruwren::foreign_v2::WrenTo for #source_ty {
                        const SCRATCH_SPACE: usize = 1;
                        #[inline]
                        fn to_vm(self, vm: &ruwren::VM, slot: ruwren::SlotId, scratch_start: ruwren::SlotId) {
                            vm.set_slot_new_foreign_scratch::<_, _, #instance_ty>(
                                module_name(),
                                #class_ty::name(),
                                self.into(),
                                slot,
                                scratch_start,
                            )
                            .unwrap();
                        }
                    }
                },
            )
        })
        .unzip();

    let expanded = quote! {
        #vis mod #name {
            use ruwren::foreign_v2::V2Class;

            #[inline]
            fn module_name() -> String {
                stringify!(#name).replace("_", "/")
            }

            #(
                #to_impls
            )*

            #[inline]
            pub fn publish_module(lib: &mut ruwren::ModuleLibrary) {
                let mut module = ruwren::Module::new();

                {
                    #(
                        #decls
                    )*
                }

                lib.module(module_name(), module);
            }
        }
    };
    
    println!("--- wren_module -----------------------------");
    writeln!(std::io::stdout(), "{}", expanded);
    proc_macro::TokenStream::from(expanded)
}
