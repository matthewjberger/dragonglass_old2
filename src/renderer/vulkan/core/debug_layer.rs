use crate::renderer::vulkan::core::Instance;
use ash::{
    extensions::ext::DebugUtils,
    vk::{
        self, Bool32, DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT,
        DebugUtilsMessengerCallbackDataEXT, DebugUtilsMessengerEXT,
    },
};
use std::{
    ffi::{CStr, CString},
    os::raw::c_void,
};

use log::{debug, error, info, trace, warn};

use snafu::{ResultExt, Snafu};

type Result<T, E = DebugLayerError> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub enum DebugLayerError {
    #[snafu(display("Failed to create debug utils messenger: {:?}", source))]
    DebugUtilsMessengerCreationFailed { source: vk::Result },
}

// TODO: Possibly rename this to DebugUtils
pub struct DebugLayer {
    debug_utils: DebugUtils,
    debug_utils_messenger: DebugUtilsMessengerEXT,
}

impl DebugLayer {
    pub fn new(instance: &Instance) -> Result<Option<Self>> {
        if !DebugLayer::validation_layers_enabled() {
            return Ok(None);
        }

        let debug_utils = DebugUtils::new(instance.entry(), instance.instance());
        let create_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
            .flags(vk::DebugUtilsMessengerCreateFlagsEXT::all())
            .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::all())
            .message_type(vk::DebugUtilsMessageTypeFlagsEXT::all())
            .pfn_user_callback(Some(vulkan_debug_callback))
            .build();
        let debug_utils_messenger = unsafe {
            debug_utils
                .create_debug_utils_messenger(&create_info, None)
                .context(DebugUtilsMessengerCreationFailed)?
        };
        Ok(Some(DebugLayer {
            debug_utils,
            debug_utils_messenger,
        }))
    }

    pub fn validation_layers_enabled() -> bool {
        cfg!(feature = "vulkan-validation") || cfg!(debug_assertions)
    }

    pub fn debug_layer_names() -> LayerNameVec {
        LayerNameVec {
            layer_names: vec![LayerName::new("VK_LAYER_LUNARG_standard_validation")],
        }
    }
}

impl Drop for DebugLayer {
    fn drop(&mut self) {
        unsafe {
            self.debug_utils
                .destroy_debug_utils_messenger(self.debug_utils_messenger, None);
        }
    }
}

pub struct LayerName {
    name: String,
    name_c_string: CString,
}

impl LayerName {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            name_c_string: CString::new(name).expect("Failed to build CString"),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn name_pointer(&self) -> *const i8 {
        self.name_c_string.as_ptr()
    }
}

impl PartialEq for LayerName {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl Eq for LayerName {}

#[derive(Default)]
pub struct LayerNameVec {
    pub layer_names: Vec<LayerName>,
}

impl LayerNameVec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn layer_name_pointers(&self) -> Vec<*const i8> {
        self.layer_names
            .iter()
            .map(|layer_name| layer_name.name_pointer())
            .collect::<Vec<_>>()
    }
}

// Setup the callback for the debug utils extension
unsafe extern "system" fn vulkan_debug_callback(
    flags: DebugUtilsMessageSeverityFlagsEXT,
    type_flags: DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const DebugUtilsMessengerCallbackDataEXT,
    _: *mut c_void,
) -> Bool32 {
    let type_flag = match type_flags {
        DebugUtilsMessageTypeFlagsEXT::GENERAL => "General",
        DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "Performance",
        DebugUtilsMessageTypeFlagsEXT::VALIDATION => "Validation",
        _ => "Unspecified",
    };

    let message = format!(
        "[{}] {:?}",
        type_flag,
        CStr::from_ptr((*p_callback_data).p_message)
    );

    match flags {
        DebugUtilsMessageSeverityFlagsEXT::ERROR => error!("{}", message),
        DebugUtilsMessageSeverityFlagsEXT::INFO => info!("{}", message),
        DebugUtilsMessageSeverityFlagsEXT::WARNING => warn!("{}", message),
        DebugUtilsMessageSeverityFlagsEXT::VERBOSE => trace!("{}", message),
        _ => debug!("{}", message),
    }

    vk::FALSE
}
